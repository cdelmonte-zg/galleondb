use anyhow::{Error, Result, bail};

use std::fs::File;
use std::os::unix::fs::FileExt;

#[derive(Debug, Clone, PartialEq)]
pub enum Value {
    Null,
    Integer(i64),
    Float(f64),
    Blob(Vec<u8>),
    Text(String),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct SerialTypeCode(u64);

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

fn read_chunk(file: &File, pos: &mut usize) -> Result<u64, Error> {
    let mut buf = [0u8; 9];

    file.read_at(&mut buf, *pos as u64)?;
    let (value, consumed) = read_varint(&buf);
    *pos += consumed;

    Ok(value)
}

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
        1 => Ok((Value::Integer(read_signed_be(&buf[..1])), 1)),
        2 => Ok((Value::Integer(read_signed_be(&buf[..2])), 2)),
        3 => Ok((Value::Integer(read_signed_be(&buf[..3])), 3)),
        4 => Ok((Value::Integer(read_signed_be(&buf[..4])), 4)),
        5 => Ok((Value::Integer(read_signed_be(&buf[..6])), 6)),
        6 => Ok((Value::Integer(read_signed_be(&buf[..8])), 8)),

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

pub fn get_db_info(file: &mut File) -> Result<(u32, u16), Error> {
    let mut header = [0u8; 100];
    let mut page_header = [0u8; 8];

    file.read_exact_at(&mut header, 0)?;
    file.read_exact_at(&mut page_header, 100)?;

    // The page size is ;stored at the 16th byte offset, using 2 bytes in big-endian order
    let page_size_encoded = u16::from_be_bytes([header[16], header[17]]);

    let page_size = match page_size_encoded {
        65_356 => 1,
        n => n as u32,
    };

    let cells_nr = u16::from_be_bytes([page_header[3], page_header[4]]);

    Ok((page_size, cells_nr))
}

pub fn get_cells(file: &mut File) -> Result<(u32, u16, Vec<Cell>), Error> {
    let (page_size, cells_nr) = get_db_info(file)?;

    // The cell pointer array consists of K 2-byte integer offsets to the cell contents.
    //  The cell pointers are arranged in key order with left-most cell (the cell with the smallest key)
    //  first and the right-most cell (the cell with the largest key) last.
    let mut cells: Vec<Cell> = vec![];
    for index in 0..cells_nr as u16 {
        let mut buf = [0u8; 2];

        let pointer_offset = 108 + u64::from(index) * 2;
        file.read_exact_at(&mut buf, pointer_offset)?;

        let offset = u16::from_be_bytes(buf);

        let mut pos = offset as usize;

        let payload_size_in_bytes = usize::try_from(read_chunk(&file, &mut pos)?)?;
        let row_id = read_chunk(&file, &mut pos)? as i64;

        let header_start = pos;

        let header_size_in_bytes = usize::try_from(read_chunk(&file, &mut pos)?)?;

        let mut serial_types = Vec::new();

        while pos - header_start < header_size_in_bytes {
            let serial_type = read_chunk(&file, &mut pos)?;
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
            file.read_at(&mut value_buf, body_pos as u64)?;

            let (value, consumed) = read_value(*serial_type, &value_buf)?;
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

    Ok((page_size, cells_nr, cells))
}

fn get_table_rootpage(file: &mut File, table_name: &str) -> Result<Option<u64>, Error> {
    let (_, _, cells) = get_cells(file)?;

    for cell in &cells {
        if let [
            Value::Text(object_type),
            Value::Text(name),
            Value::Text(_tbl_name),
            Value::Integer(rootpage),
            Value::Text(_sql),
        ] = cell.record.body.as_slice()
        {
            if object_type == "table" && name.eq(table_name) {
                return Ok(Some(u64::try_from(*rootpage)?));
            }
        }
    }

    bail!("No page root found for table {}", table_name)
}
