use crate::{bind::BindAdapter, descriptor::RowExtractor, dialect::SqlDialect};

pub trait Backend: Send + Sync + 'static {
    type Database: sqlx::Database;
    type Adapter: BindAdapter<Self::Database> + Send + Sync + 'static;
    type Extractor: RowExtractor + Default + Send + Sync + 'static;

    fn dialect() -> SqlDialect {
        <Self::Adapter as BindAdapter<Self::Database>>::dialect()
    }
}

pub trait DbBound: Backend {}

pub type DbOf<B> = <B as Backend>::Database;
pub type AdOf<B> = <B as Backend>::Adapter;
pub type ExOf<B> = <B as Backend>::Extractor;
pub type RowOf<B> = <ExOf<B> as RowExtractor>::Row;
