/// Check if a sqlx error is a unique constraint violation (PG error code 23505).
pub fn is_unique_violation(e: &sqlx::Error) -> bool {
    if let sqlx::Error::Database(db_err) = e {
        db_err.code().as_deref() == Some("23505")
    } else {
        false
    }
}

/// Extract the constraint name from a unique violation error, if available.
pub fn violated_constraint(e: &sqlx::Error) -> Option<&str> {
    if let sqlx::Error::Database(db_err) = e {
        db_err.constraint()
    } else {
        None
    }
}
