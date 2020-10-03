use crate::Scalar;
use serde::{Serialize, Deserialize};

/// Uploaded file.
///
/// **Reference:** <https://github.com/jaydenseric/graphql-multipart-request-spec>
///
/// Graphql supports file uploads via `multipart/form-data`.
/// Enable this feature by accepting an argument of type `Upload` (single file) or
/// `Vec<Upload>` (multiple files) in your mutation like in the example blow.
///
/// # Example
/// *[Full Example](<https://github.com/async-graphql/examples/blob/master/models/files/src/lib.rs>)*
///
/// ```
/// use async_graphql::*;
///
/// struct MutationRoot;
///
/// #[Object]
/// impl MutationRoot {
///     async fn upload(&self, file: Upload) -> bool {
///         println!("upload: filename={}", file.filename());
///         true
///     }
/// }
///
/// ```
/// # Example Curl Request
/// Assuming you have defined your MutationRoot like in the example above,
/// you can now upload a file `myFile.txt` with the below curl command:
///
/// ```curl
/// curl 'localhost:8000' \
/// --form 'operations={
///         "query": "mutation ($file: Upload!) { upload(file: $file)  }",
///         "variables": { "file": null }}' \
/// --form 'map={ "0": ["variables.file"] }' \
/// --form '0=@myFile.txt'
/// ```
#[derive(Serialize, Deserialize, Scalar)]
#[graphql(internal)]
#[serde(transparent)]
pub struct Upload(pub String);
