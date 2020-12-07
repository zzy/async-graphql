use std::pin::Pin;
use std::str::FromStr;

use async_graphql::http::{WebSocket as AGWebSocket, WebSocketProtocols};
use async_graphql::{Data, ObjectType, Result, Schema, SubscriptionType};
use futures_util::{future, StreamExt};
use tide::{Endpoint, Request, Response};
use tide_websockets::Message;

/// GraphQL subscription endpoint.
#[cfg_attr(feature = "nightly", doc(cfg(feature = "unstable")))]
pub struct WebSocket<S> {
    inner: Pin<Box<dyn Endpoint<S>>>,
}

#[async_trait::async_trait]
impl<S> Endpoint<S> for WebSocket<S>
where
    S: Send + Sync + Clone + 'static,
{
    async fn call(&self, req: Request<S>) -> tide::Result<Response> {
        self.inner.call(req).await
    }
}

impl<S> WebSocket<S>
where
    S: Send + Sync + Clone + 'static,
{
    /// Create a graphql subscription endpoint.
    pub fn new<Query, Mutation, Subscription>(schema: Schema<Query, Mutation, Subscription>) -> Self
    where
        Query: ObjectType + Send + Sync + 'static,
        Mutation: ObjectType + Send + Sync + 'static,
        Subscription: SubscriptionType + Send + Sync + 'static,
    {
        Self::new_with_initializer::<fn(serde_json::Value) -> Result<Data>, _, _, _>(schema, None)
    }

    /// Create a graphql subscription endpoint.
    ///
    /// Specifies that a function converts the init payload to data.
    pub fn new_with_initializer<F, Query, Mutation, Subscription>(
        schema: Schema<Query, Mutation, Subscription>,
        initializer: Option<F>,
    ) -> Self
    where
        Query: ObjectType + Send + Sync + 'static,
        Mutation: ObjectType + Send + Sync + 'static,
        Subscription: SubscriptionType + Send + Sync + 'static,
        F: FnOnce(serde_json::Value) -> Result<Data> + Send + Sync + Clone + 'static,
    {
        let endpoint = tide_websockets::WebSocket::<S, _>::new(move |request, connection| {
            let schema = schema.clone();
            let initializer = initializer.clone();
            async move {
                let protocol = request
                    .header("sec-websocket-protocol")
                    .map(|value| value.as_str())
                    .and_then(|value| WebSocketProtocols::from_str(value).ok())
                    .unwrap_or_default();

                let sink = connection.clone();
                let mut stream = AGWebSocket::with_data(
                    schema.clone(),
                    connection
                        .take_while(|msg| future::ready(msg.is_ok()))
                        .map(Result::unwrap)
                        .map(Message::into_data),
                    initializer,
                    protocol,
                );
                while let Some(data) = stream.next().await {
                    if let Err(_) = sink.send_string(data).await {
                        break;
                    }
                }

                Ok(())
            }
        })
        .with_protocols(&["graphql-transport-ws", "graphql-ws"]);
        Self {
            inner: Box::pin(endpoint),
        }
    }
}
