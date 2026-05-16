pub trait SoftDeletable {
    const SOFT_DELETE_COLUMN: &'static str = "deleted_at";
}
