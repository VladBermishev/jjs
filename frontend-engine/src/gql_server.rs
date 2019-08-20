mod auth;
mod context;
mod misc;
mod queries;
mod runs;
mod schema;
mod users;

type ErrorExtension = juniper::Value<juniper::DefaultScalarValue>;

struct ApiError {
    visible: bool,
    extension: Option<ErrorExtension>,
    source: Option<Box<dyn std::error::Error>>,
    ctx: Context,
}

mod impl_display {
    use super::*;
    use std::fmt::{self, Display, Formatter};

    impl Display for ApiError {
        fn fmt(&self, f: &mut Formatter) -> fmt::Result {
            write!(f, "Frontend error")?;
            if self.visible {
                write!(f, "(pub) ")?;
            } else {
                write!(f, "(priv) ")?;
            }
            if let Some(ext) = &self.extension {
                write!(f, "[{:?}]", ext)?;
            }
            if let Some(src) = &self.source {
                write!(f, ": {}", src)?;
            }
            Ok(())
        }
    }
}

struct EmptyError;

impl std::fmt::Display for EmptyError {
    fn fmt(&self, _f: &mut std::fmt::Formatter) -> std::fmt::Result {
        Ok(())
    }
}

impl std::fmt::Debug for EmptyError {
    fn fmt(&self, _f: &mut std::fmt::Formatter) -> std::fmt::Result {
        Ok(())
    }
}

impl std::error::Error for EmptyError {}

impl juniper::IntoFieldError for ApiError {
    fn into_field_error(self) -> juniper::FieldError {
        let data: &dyn std::error::Error = match &self.source {
            Some(err) if self.visible => &**err,
            _ => {
                if let Some(err) = self.source {
                    let err_msg = err.to_string();
                    slog::error!(
                        self.ctx.logger,
                        "Error when processing api request: {error}",
                        error = &err_msg
                    );
                }
                &EmptyError
            }
        };
        juniper::FieldError::new(data, self.extension.unwrap_or_else(juniper::Value::null))
    }
}

type ApiResult<T> = Result<T, ApiError>;

trait ResultToApiUtil<T, E> {
    /// Handle error as internal, if any
    fn internal(self, ctx: &Context) -> Result<T, ApiError>;

    /// Show error to user, if any
    fn report(self, ctx: &Context) -> Result<T, ApiError>;

    /// like `report`, but also return extension
    fn report_ext(self, ctx: &Context, ext: ErrorExtension) -> Result<T, ApiError>;

    /// like 'report_ext', but produce extension from error with supplied callback
    fn report_with(
        self,
        ctx: &Context,
        make_ext: impl FnOnce(&E) -> ErrorExtension,
    ) -> Result<T, ApiError>;
}

impl<T, E: std::error::Error + 'static> ResultToApiUtil<T, E> for Result<T, E> {
    fn internal(self, ctx: &Context) -> Result<T, ApiError> {
        self.map_err(|err| ApiError {
            visible: false,
            extension: None,
            source: Some(Box::new(err)),
            ctx: ctx.clone(),
        })
    }

    fn report(self, ctx: &Context) -> Result<T, ApiError> {
        self.map_err(|err| ApiError {
            visible: true,
            extension: None,
            source: Some(Box::new(err)),
            ctx: ctx.clone(),
        })
    }

    fn report_ext(self, ctx: &Context, ext: ErrorExtension) -> Result<T, ApiError> {
        self.map_err(|err| ApiError {
            visible: true,
            extension: Some(ext),
            source: Some(Box::new(err)),
            ctx: ctx.clone(),
        })
    }

    fn report_with(
        self,
        ctx: &Context,
        make_ext: impl FnOnce(&E) -> ErrorExtension,
    ) -> Result<T, ApiError> {
        self.map_err(|err| ApiError {
            visible: true,
            extension: Some(make_ext(&err)),
            source: Some(Box::new(err)),
            ctx: ctx.clone(),
        })
    }
}

trait StrErrorMsgUtil {
    fn report<T>(&self, ctx: &Context) -> Result<T, ApiError>;
}

impl StrErrorMsgUtil for str {
    fn report<T>(&self, ctx: &Context) -> Result<T, ApiError> {
        Err(ApiError {
            visible: true,
            extension: Some(self.into()),
            source: None,
            ctx: ctx.clone(),
        })
    }
}

mod prelude {
    pub(super) use super::{
        schema, ApiError, ApiResult, Context, ErrorExtension, ResultToApiUtil as _,
        StrErrorMsgUtil as _,
    };
}

pub(crate) use context::{Context, ContextFactory};

pub(crate) struct Query;

pub(crate) struct Mutation;

pub(crate) type Schema = juniper::RootNode<'static, Query, Mutation>;
