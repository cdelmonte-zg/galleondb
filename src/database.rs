use anyhow::{Context, Error, Result, bail};

use std::cmp::Ordering;
use std::fs::File;
use std::os::unix::fs::FileExt;

use crate::value;

use super::value::Value;

#[derive(Debug)]
pub struct Database {
    pub file: File,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct SerialTypeCode(u64);

// 0       NULL                  0 byte
// 1       signed integer         1 byte
// 2       signed integer         2 byte
// 3       signed integer         3 byte
// 4       signed integer         4 byte
// 5       signed integer         6 byte
// 6       signed integer         8 byte
// 7       IEEE 754 float         8 byte
// 8       integer constant 0     0 byte
// 9       integer constant 1     0 byte
// 10,11   reserved
// >=12 even  BLOB                (N - 12) / 2 bytes
// >=13 odd   TEXT                (N - 13) / 2 bytes
impl SerialTypeCode {
    fn payload_size(self) -> Result<usize, Error> {
        match self.0 {
            0 => Ok(0),
            1 => Ok(1),
            2 => Ok(2),
            3 => Ok(3),
            4 => Ok(4),
            5 => Ok(6),
            6 => Ok(8),
            7 => Ok(8),
            8 => Ok(0),
            9 => Ok(0),
            n if n >= 12 && n % 2 == 0 => Ok(((n - 12) / 2) as usize),
            n if n >= 13 && n % 2 == 1 => Ok(((n - 13) / 2) as usize),

            n => bail!("Invalid od reserved serial type code: {}", n),
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
struct RecordHeader {
    size_in_bytes: usize,
    serial_types: Vec<SerialTypeCode>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct RowRecord {
    header: RecordHeader,
    pub body: Vec<Value>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Cell {
    pub offset: u16,
    pub payload_size: usize,
    pub row_id: i64,
    pub record: RowRecord,
}

#[derive(Debug, Clone, PartialEq)]
struct BTreePageHeader {
    page_type: u8,
    cell_count: u16,
    cell_content_area_start: u16,
    cell_pointer_array_start: u64,
    right_most_child: Option<u32>,
}

#[derive(Clone, Debug, PartialEq)]
struct IndexInteriorPage {
    cells: Vec<IndexInteriorCell>,
    right_most_child: u64,
}

#[derive(Clone, Debug, PartialEq)]
struct IndexInteriorCell {
    left_child_page: u64,
    record: IndexRecord,
}

#[derive(Clone, Debug, PartialEq)]
struct IndexLeafCell {
    offset: u16,
    payload_size: usize,
    record: IndexRecord,
}

#[derive(Clone, Debug, PartialEq)]
struct IndexRecord {
    values: Vec<Value>,
    row_id: i64,
}

impl Database {
    fn read_varint(buf: &[u8]) -> (u64, usize) {
        let mut result = 0 as u64;

        for i in 0..9 {
            let byte = buf[i];

            //ninth byte is taken as it is
            if i == 8 {
                result = (result << 8) | byte as u64;
                return (result, 9);
            }

            // takes value part
            result = (result << 7) | (byte & 0x7f) as u64;

            // verifies if the variant continues
            if byte & 0x80 == 0 {
                return (result, i + 1);
            }
        }

        unreachable!()
    }

    fn read_signed_be(bytes: &[u8]) -> i64 {
        let mut raw = 0u64;

        // perchè &byte e non byte??
        for &byte in bytes {
            raw = (raw << 8) | byte as u64;
        }

        let bits = bytes.len() * 8;

        if bits == 64 {
            return raw as i64;
        }

        let sign_bit = 1u64 << (bits - 1);

        if raw & sign_bit == 0 {
            raw as i64
        } else {
            let extension_mask = !0u64 << bits;
            (raw | extension_mask) as i64
        }
    }

    fn read_value(serial_type: SerialTypeCode, buf: &[u8]) -> Result<(Value, usize)> {
        match serial_type.0 {
            0 => Ok((Value::Null, 0)),
            1 => Ok((Value::Integer(Database::read_signed_be(&buf[..1])), 1)),
            2 => Ok((Value::Integer(Database::read_signed_be(&buf[..2])), 2)),
            3 => Ok((Value::Integer(Database::read_signed_be(&buf[..3])), 3)),
            4 => Ok((Value::Integer(Database::read_signed_be(&buf[..4])), 4)),
            5 => Ok((Value::Integer(Database::read_signed_be(&buf[..6])), 6)),
            6 => Ok((Value::Integer(Database::read_signed_be(&buf[..8])), 8)),

            7 => {
                let bytes: [u8; 8] = buf[..8].try_into()?;
                let bits = u64::from_be_bytes(bytes);
                Ok((Value::Float(f64::from_bits(bits)), 8))
            }

            8 => Ok((Value::Integer(0), 0)),
            9 => Ok((Value::Integer(1), 0)),

            n if n >= 12 && n % 2 == 0 => {
                let len = ((n - 12) / 2) as usize;
                Ok((Value::Blob(buf[..len].to_vec()), len))
            }

            n if n >= 13 && n % 2 == 1 => {
                let len = ((n - 13) / 2) as usize;
                let text = String::from_utf8(buf[..len].to_vec())?;
                Ok((Value::Text(text), len))
            }

            n => bail!("invalid or reserved serial type code: {}", n),
        }
    }

    fn read_chunk(&self, pos: &mut usize) -> Result<u64, Error> {
        let mut buf = [0u8; 9];

        self.file.read_exact_at(&mut buf, *pos as u64)?;
        let (value, consumed) = Database::read_varint(&buf);
        *pos += consumed;

        Ok(value)
    }

    pub fn get_db_info(&self) -> Result<u32, Error> {
        let mut header = [0u8; 100];
        self.file.read_exact_at(&mut header, 0)?;

        // The page size is ;stored at the 16th byte offset, using 2 bytes in big-endian order
        let page_size_encoded = u16::from_be_bytes([header[16], header[17]]);

        Ok(match page_size_encoded {
            1 => 65_536,
            n => n as u32,
        })
    }

    pub fn read_schema_cells(&self) -> Result<(u32, usize, Vec<Cell>), Error> {
        let page_size = self.get_db_info()?;
        let cells = self.read_table(1, page_size)?;

        Ok((page_size, cells.len(), cells))
    }

    pub fn lookup_index(
        &self,
        page_number: u64,
        page_size: u32,
        search_value: &Value,
    ) -> anyhow::Result<Vec<i64>> {
        let header = self.read_btree_page_header(page_number, page_size)?;

        match header.page_type {
            0x0A => {
                let cells = self.read_leaf_index_cells(page_number, page_size)?;

                Ok(cells
                    .into_iter()
                    .filter(|cell| cell.record.values.first() == Some(search_value))
                    .map(|cell| cell.record.row_id)
                    .collect())
            }

            0x02 => {
                let page = self.read_index_interior_page(page_number, page_size)?;

                let mut row_ids = Vec::new();

                for cell in page.cells {    
                    let key = cell.record.values.first().context("Index record has no key")?;

                    match Database::compare(search_value, key)? {
                        Ordering::Greater => {
                            continue;
                        }

                        Ordering::Equal => {
                            row_ids.extend(
                                self.lookup_index(cell.left_child_page, page_size, search_value)?
                            );

                            row_ids.push(cell.record.row_id);
                        }

                        Ordering::Less => {
                            row_ids.extend(
                                self.lookup_index(cell.left_child_page, page_size, search_value)?
                            );

                            return Ok(row_ids);
                        }
                    }
                }

                row_ids.extend(self.lookup_index(page.right_most_child, page_size, search_value)?);

                Ok(row_ids)
            }
            
            _ => bail!("Wrong page type for lookup index"),
        }
    }


    fn compare(search_value: &Value, key: &Value) -> anyhow::Result<Ordering> {
        match (search_value, key) {
            (Value::Text(left), Value::Text(right)) => Ok(left.cmp(right)),
            
            (Value::Text(_), Value::Null) => {
                Ok(Ordering::Greater)
            }

            (Value::Null, Value::Text(_)) => {
                Ok(Ordering::Less)
            }

            (Value::Null, Value::Null) => {
                Ok(Ordering::Equal)
            }

            _ => bail!(
                "Unsupported index comparison: {:?} vs {:?}",
                search_value,
                key
            ), 
        }

    }

    fn read_index_interior_page(
        &self,
        page_number: u64,
        page_size: u32,
    ) -> anyhow::Result<IndexInteriorPage> {
        let page_start = (page_number - 1) * page_size as u64;

        let header = self.read_btree_page_header(page_number, page_size)?;

        if header.page_type != 0x02 {
            bail!(
                "Expected index interior page, found {:#04x}",
                header.page_type
            );
        }

        let mut cells = vec![];

        for index in 0..header.cell_count {
            let mut buf = [0u8; 2];

            let pointer_offset = header.cell_pointer_array_start + u64::from(index) * 2;

            self.file.read_exact_at(&mut buf, pointer_offset)?;

            let offset = u16::from_be_bytes(buf);
            let cell_start = page_start + u64::from(offset);

            let mut buf = [0u8; 4];
            self.file.read_exact_at(&mut buf, cell_start)?;
            let left_child_page = u64::from(u32::from_be_bytes(buf));

            let mut pos = cell_start as usize + 4;

            let payload_size_in_bytes = usize::try_from(self.read_chunk(&mut pos)?)?;

            let header_start = pos;

            let header_size_in_bytes = usize::try_from(self.read_chunk(&mut pos)?)?;

            let mut serial_types = Vec::new();

            while pos - header_start < header_size_in_bytes {
                let serial_type = self.read_chunk(&mut pos)?;
                serial_types.push(SerialTypeCode(serial_type));
            }

            let record_header = RecordHeader {
                size_in_bytes: header_size_in_bytes,
                serial_types,
            };

            let body_start = header_start + header_size_in_bytes;
            let mut body_pos = body_start;

            let mut values = Vec::new();

            for serial_type in &record_header.serial_types {
                let len = serial_type.payload_size()?;

                let mut value_buf: Vec<u8> = vec![0u8; len];
                self.file.read_exact_at(&mut value_buf, body_pos as u64)?;

                let (value, consumed) = Database::read_value(*serial_type, &value_buf)?;
                debug_assert_eq!(consumed, len);

                values.push(value);

                body_pos += len;
            }

            let row_id = match values.pop() {
                Some(Value::Integer(row_id)) => row_id,
                _ => bail!("Expected at least 1 value as row_id"),
            };

            cells.push(IndexInteriorCell {
                left_child_page,
                record: IndexRecord { values, row_id },
            });
        }

        let right_most_child = match header.right_most_child {
            Some(page) => u64::from(page),
            _ => bail!("No most right child found"),
        };

        Ok(IndexInteriorPage {
            cells,
            right_most_child,
        })
    }

    pub(crate) fn read_table_row(
        &self,
        page_number: u64,
        page_size: u32,
        row_id: i64,
    ) -> anyhow::Result<Cell> {
        let header = self.read_btree_page_header(page_number, page_size)?;

        match header.page_type {
            0x0D => self
                .read_leaf_page_cells(page_number, page_size)?
                .into_iter()
                .find(|cell| cell.row_id == row_id)
                .context("Row not found"),
            0x05 => {
                let child = self.find_table_child(page_number, page_size, row_id)?;

                self.read_table_row(child, page_size, row_id)
            }

            _ => bail!("Expected B-Tree Table, found {:#04x}", header.page_type),
        }
    }

    fn find_table_child(
        &self,
        page_number: u64,
        page_size: u32,
        row_id: i64,
    ) -> anyhow::Result<u64> {
        let page_start = (page_number - 1) * page_size as u64;
        let header = self.read_btree_page_header(page_number, page_size)?;

        if header.page_type != 0x05 {
            bail!("Expected 0x05 page, found {:#04x}", header.page_type);
        }

        for index in 0..header.cell_count {
            let mut buf = [0u8; 2];
            let pointer_offset = header.cell_pointer_array_start + (u64::from(index)) * 2;
            self.file.read_exact_at(&mut buf, pointer_offset)?;
            let cell_offset = u16::from_be_bytes(buf);

            let cell_start = page_start + u64::from(cell_offset);
            let mut buf = [0u8; 4];
            self.file.read_exact_at(&mut buf, cell_start)?;
            let left_child_page = u32::from_be_bytes(buf);

            let mut pos = cell_start as usize + 4;
            let separator_row_id = self.read_chunk(&mut pos)? as i64;

            if row_id <= separator_row_id {
                return Ok(u64::from(left_child_page));
            }
        }

        let right_most_child = header
            .right_most_child
            .context("Missing right-most child")?;

        Ok(u64::from(right_most_child))
    }

    fn read_leaf_index_cells(
        &self,
        page_number: u64,
        page_size: u32,
    ) -> anyhow::Result<Vec<IndexLeafCell>> {
        let page_start = (page_number - 1) * page_size as u64;

        let btree_page_header = self.read_btree_page_header(page_number, page_size)?;

        if btree_page_header.page_type != 0x0A {
            bail!(
                "Expected index leaf page, found {:#04x}",
                btree_page_header.page_type
            );
        }

        let mut cells = vec![];
        for index in 0..btree_page_header.cell_count {
            let mut buf = [0u8; 2];

            let pointer_offset = btree_page_header.cell_pointer_array_start + u64::from(index) * 2;

            self.file.read_exact_at(&mut buf, pointer_offset)?;

            let offset = u16::from_be_bytes(buf);
            let cell_start = page_start + u64::from(offset);
            let mut pos = cell_start as usize;

            let payload_size_in_bytes = usize::try_from(self.read_chunk(&mut pos)?)?;

            let header_start = pos;

            let header_size_in_bytes = usize::try_from(self.read_chunk(&mut pos)?)?;

            let mut serial_types = Vec::new();

            while pos - header_start < header_size_in_bytes {
                let serial_type = self.read_chunk(&mut pos)?;
                serial_types.push(SerialTypeCode(serial_type));
            }

            let record_header = RecordHeader {
                size_in_bytes: header_size_in_bytes,
                serial_types,
            };

            let body_start = header_start + header_size_in_bytes;
            let mut body_pos = body_start;

            let mut values = Vec::new();

            for serial_type in &record_header.serial_types {
                let len = serial_type.payload_size()?;

                let mut value_buf: Vec<u8> = vec![0u8; len];
                self.file.read_exact_at(&mut value_buf, body_pos as u64)?;

                let (value, consumed) = Database::read_value(*serial_type, &value_buf)?;
                debug_assert_eq!(consumed, len);

                values.push(value);

                body_pos += len;
            }

            let row_id = match values.pop() {
                Some(Value::Integer(row_id)) => row_id,
                _ => bail!("Expected at least 1 value as row_id"),
            };

            cells.push(IndexLeafCell {
                offset,
                payload_size: payload_size_in_bytes,
                record: IndexRecord { values, row_id },
            });
        }

        Ok(cells)
    }

    pub fn read_table(&self, page_number: u64, page_size: u32) -> anyhow::Result<Vec<Cell>> {
        let header = self.read_btree_page_header(page_number, page_size)?;

        match header.page_type {
            0x0D => self.read_leaf_page_cells(page_number, page_size),

            0x05 => {
                let children = self.read_table_interior_children(page_number, page_size)?;

                let mut cells = Vec::new();

                for child_page in children {
                    cells.extend(self.read_table(child_page, page_size)?);
                }

                Ok(cells)
            }

            _ => bail!("Page type {} not yet implemented", header.page_type),
        }
    }

    fn read_btree_page_header(
        &self,
        page_number: u64,
        page_size: u32,
    ) -> anyhow::Result<BTreePageHeader> {
        let page_start = (page_number - 1) * page_size as u64;

        let header_start = if page_number == 1 {
            page_start + 100
        } else {
            page_start
        };

        let mut header = [0u8; 12];
        self.file.read_exact_at(&mut header, header_start)?;

        // 0x02 → index interior
        // 0x05 → table interior
        // 0x0A → index leaf
        // 0x0D → table leaf
        let page_type = header[0];

        let cell_count = u16::from_be_bytes([header[3], header[4]]);
        let cell_content_area_start = u16::from_be_bytes([header[5], header[6]]);
        let (header_size, right_most_child) = match page_type {
            0x0A | 0x0D => (8, None),
            0x02 | 0x05 => (
                12,
                Some(u32::from_be_bytes([
                    header[8], header[9], header[10], header[11],
                ])),
            ),
            _ => bail!("Invalid B-tree page type: {page_type:#04x}"),
        };
        let cell_pointer_array_start = header_size + header_start;

        Ok(BTreePageHeader {
            page_type,
            cell_count,
            cell_content_area_start,
            cell_pointer_array_start,
            right_most_child,
        })
    }

    fn read_leaf_page_cells(&self, page_number: u64, page_size: u32) -> anyhow::Result<Vec<Cell>> {
        let page_start = (page_number - 1) * page_size as u64;
        let btree_page_header = self.read_btree_page_header(page_number, page_size)?;

        if btree_page_header.page_type != 0x0D {
            bail!(
                "Expected table leaf page, found {:#04x}",
                btree_page_header.page_type
            );
        }

        // The cell pointer array consists of K 2-byte integer offsets to the cell contents.
        //  The cell pointers are arranged in key order with left-most cell (the cell with the smallest key)
        //  first and the right-most cell (the cell with the largest key) last.
        let mut cells: Vec<Cell> = vec![];
        for index in 0..btree_page_header.cell_count {
            let mut buf = [0u8; 2];

            let pointer_offset = btree_page_header.cell_pointer_array_start + u64::from(index) * 2;

            self.file.read_exact_at(&mut buf, pointer_offset)?;

            let offset = u16::from_be_bytes(buf);
            let cell_start = page_start + u64::from(offset);
            let mut pos = cell_start as usize;

            let payload_size_in_bytes = usize::try_from(self.read_chunk(&mut pos)?)?;
            let row_id = self.read_chunk(&mut pos)? as i64;

            let header_start = pos;

            let header_size_in_bytes = usize::try_from(self.read_chunk(&mut pos)?)?;

            let mut serial_types = Vec::new();

            while pos - header_start < header_size_in_bytes {
                let serial_type = self.read_chunk(&mut pos)?;
                serial_types.push(SerialTypeCode(serial_type));
            }

            let record_header = RecordHeader {
                size_in_bytes: header_size_in_bytes,
                serial_types,
            };

            let body_start = header_start + header_size_in_bytes;
            let mut body_pos = body_start;

            let mut values = Vec::new();

            for serial_type in &record_header.serial_types {
                let len = serial_type.payload_size()?;

                let mut value_buf: Vec<u8> = vec![0u8; len];
                self.file.read_exact_at(&mut value_buf, body_pos as u64)?;

                let (value, consumed) = Database::read_value(*serial_type, &value_buf)?;
                debug_assert_eq!(consumed, len);

                values.push(value);

                body_pos += len;
            }

            cells.push(Cell {
                offset,
                payload_size: payload_size_in_bytes,
                row_id,
                record: RowRecord {
                    header: record_header,
                    body: values,
                },
            });
        }

        Ok(cells)
    }

    pub fn read_table_interior_children(
        &self,
        page_number: u64,
        page_size: u32,
    ) -> Result<Vec<u64>> {
        let page_start = (page_number - 1) * page_size as u64;
        let header = self.read_btree_page_header(page_number, page_size)?;

        if header.page_type != 0x05 {
            bail!(
                "Expected table interior children node, found {:#04x}",
                header.page_type
            );
        }

        let mut children = Vec::new();
        for index in 0..header.cell_count {
            let pointer_offset = header.cell_pointer_array_start + u64::from(index) * 2;

            let mut buf = [0u8; 2];
            self.file.read_exact_at(&mut buf, pointer_offset)?;
            let cell_offset = u16::from_be_bytes(buf);
            let cell_start = page_start + u64::from(cell_offset);

            let mut buf = [0u8; 4];
            self.file.read_exact_at(&mut buf, cell_start)?;
            let child = u32::from_be_bytes(buf);

            children.push(u64::from(child));
        }

        children.push(u64::from(
            header
                .right_most_child
                .context("Missing right most child")?,
        ));

        Ok(children)
    }

    pub fn leaf_page_cell_count(&self, rootpage: &i64, page_size: u32) -> anyhow::Result<u16> {
        let offset = (*rootpage as u64 - 1) * page_size as u64;
        let mut page_header = [0u8; 4];
        self.file.read_exact_at(&mut page_header, offset + 3)?;
        let num_rows = u16::from_be_bytes([page_header[0], page_header[1]]);

        Ok(num_rows)
    }
}
