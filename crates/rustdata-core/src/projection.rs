use crate::{column::ColumnDef, descriptor::RowExtractor, error::RepositoryError};

pub trait Projection: Sized {
    type Entity;

    fn columns() -> &'static [ColumnDef];

    fn from_row<E: RowExtractor>(
        row: &E::Row,
        ext: &E,
    ) -> Result<Self, RepositoryError>;
}
