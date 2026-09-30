pub(crate) mod executor;
pub(crate) mod row;
pub(crate) use row::Row;

#[derive(Debug)]
pub enum ExecutionResult {
    Shell(Vec<String>),
    Query(Vec<Row>)
}