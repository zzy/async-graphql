use std::borrow::Cow;
use std::collections::{VecDeque, LinkedList, HashSet, BTreeSet};

use crate::resolver_utils::resolve_list;
use crate::parser::types::Field;
use crate::{Type, InputValueType, ServerResult, ContextSelectionSet, Positioned, registry, OutputValueType};

macro_rules! impl_for_listlikes {
    ($($t:ident$(: $bound:path)?),*) => {
        $(
            impl<T: Type> Type for $t<T> {
                fn type_name() -> Cow<'static, str> {
                    Cow::Owned(format!("[{}]", T::qualified_type_name()))
                }
            
                fn qualified_type_name() -> String {
                    format!("[{}]!", T::qualified_type_name())
                }
            
                fn create_type_info(registry: &mut registry::Registry) -> String {
                    T::create_type_info(registry);
                    Self::qualified_type_name()
                }
            }
            
            impl<T: InputValueType + Ord $(+ $bound)?> InputValueType for $t<T> {}
            
            #[async_trait::async_trait]
            impl<T: OutputValueType + Send + Sync + Ord> OutputValueType for $t<T> {
                async fn resolve(
                    &self,
                    ctx: &ContextSelectionSet<'_>,
                    field: &Positioned<Field>,
                ) -> ServerResult<serde_json::Value> {
                    resolve_list(ctx, field, self).await
                }
            }
        )*
    }   
}

impl_for_listlikes!(Vec, VecDeque, LinkedList, HashSet: std::hash::Hash, BTreeSet);

impl<'a, T: Type + 'a> Type for &'a [T] {
    fn type_name() -> Cow<'static, str> {
        Cow::Owned(format!("[{}]", T::qualified_type_name()))
    }

    fn qualified_type_name() -> String {
        format!("[{}]!", T::qualified_type_name())
    }

    fn create_type_info(registry: &mut registry::Registry) -> String {
        T::create_type_info(registry);
        Self::qualified_type_name()
    }
}

#[async_trait::async_trait]
impl<T: OutputValueType + Send + Sync> OutputValueType for &[T] {
    async fn resolve(
        &self,
        ctx: &ContextSelectionSet<'_>,
        field: &Positioned<Field>,
    ) -> ServerResult<serde_json::Value> {
        resolve_list(ctx, field, self.iter()).await
    }
}
