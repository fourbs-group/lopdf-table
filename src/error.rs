//! Error types for the lopdf-table library

use thiserror::Error;

/// Result type alias using TableError
pub type Result<T> = std::result::Result<T, TableError>;

/// Errors that can occur when working with tables
#[derive(Debug, Error)]
pub enum TableError {
    /// Error from the underlying lopdf library
    #[error("PDF operation failed: {0}")]
    PdfError(#[from] lopdf::Error),

    /// Invalid table structure
    #[error("Invalid table structure: {0}")]
    InvalidTable(String),

    /// Layout calculation error
    #[error("Layout calculation failed: {0}")]
    LayoutError(String),

    /// Invalid styling configuration
    #[error("Invalid style configuration: {0}")]
    StyleError(String),

    /// Text rendering error
    #[error("Text rendering failed: {0}")]
    TextError(String),

    /// Invalid dimensions
    #[error("Invalid dimensions: {0}")]
    DimensionError(String),

    /// Page not found
    #[error("Page with ID {0:?} not found")]
    PageNotFound(lopdf::ObjectId),
}
