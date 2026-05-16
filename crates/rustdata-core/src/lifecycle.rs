use crate::error::RepositoryError;

pub trait LifecycleHooks<E>: Send + Sync {
    fn before_save(_entity: &mut E) -> Result<(), RepositoryError> {
        Ok(())
    }

    fn after_save(_entity: &E) -> Result<(), RepositoryError> {
        Ok(())
    }
}
