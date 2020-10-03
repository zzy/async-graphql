use crate::parser::types::Field;
use crate::{Type, OutputValueType, ContextSelectionSet, Positioned, ServerResult};
use crate::registry::Registry;
use std::borrow::Cow;
use async_trait::async_trait;

impl<'a> Type for &'a str {
    fn type_name() -> Cow<'static, str> {
        String::type_name()
    }
    
    fn create_type_info(registry: &mut Registry) -> String {
        String::create_type_info(registry)
    }
}

#[async_trait]
impl<'a> OutputValueType for &'a str {
    async fn resolve(
        &self,
        ctx: &ContextSelectionSet<'_>,
        _field: &Positioned<Field>
    ) -> ServerResult<serde_json::Value> {
        Ok(serde_json::Value::String(self.to_owned()))
    }
}
