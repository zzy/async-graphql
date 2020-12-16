use async_graphql::*;

#[async_std::test]
pub async fn test_interface_simple_object() {
    #[derive(SimpleObject)]
    struct MyObj {
        id: i32,
        title: String,
    }

    #[derive(Interface)]
    #[graphql(field(name = "id", type = "&i32"))]
    enum Node {
        MyObj(MyObj),
    }

    struct Query;

    #[Object]
    impl Query {
        async fn node(&self) -> Node {
            MyObj {
                id: 33,
                title: "haha".to_string(),
            }
            .into()
        }
    }

    let query = r#"{
            node {
                id
                ... on Node {
                    id2: id
                }
            }
        }"#;
    let schema = Schema::new(Query, EmptyMutation, EmptySubscription);
    assert_eq!(
        schema.execute(query).await.into_result().unwrap().data,
        value!({
            "node": {
                "id": 33,
                "id2": 33,
            }
        })
    );
}

#[async_std::test]
pub async fn test_interface_simple_object2() {
    #[derive(SimpleObject)]
    struct MyObj {
        id: i32,
        title: String,
    }

    #[derive(Interface)]
    #[graphql(field(name = "id", type = "&i32"))]
    enum Node {
        MyObj(MyObj),
    }

    struct Query;

    #[Object]
    impl Query {
        async fn node(&self) -> Node {
            MyObj {
                id: 33,
                title: "haha".to_string(),
            }
            .into()
        }
    }

    let query = r#"{
            node {
                id
                ... on Node {
                    id2: id
                }
            }
        }"#;
    let schema = Schema::new(Query, EmptyMutation, EmptySubscription);
    assert_eq!(
        schema.execute(query).await.into_result().unwrap().data,
        value!({
            "node": {
                "id": 33,
                "id2": 33,
            }
        })
    );
}

#[async_std::test]
pub async fn test_multiple_interfaces() {
    struct MyObj;

    #[Object]
    impl MyObj {
        async fn value_a(&self) -> i32 {
            1
        }

        async fn value_b(&self) -> i32 {
            2
        }

        async fn value_c(&self) -> i32 {
            3
        }
    }

    #[derive(Interface)]
    #[graphql(field(name = "value_a", type = "i32"))]
    enum InterfaceA {
        MyObj(MyObj),
    }

    #[derive(Interface)]
    #[graphql(field(name = "value_b", type = "i32"))]
    enum InterfaceB {
        MyObj(MyObj),
    }

    struct Query;

    #[Object]
    impl Query {
        async fn my_obj(&self) -> InterfaceB {
            MyObj.into()
        }
    }

    let schema = Schema::build(Query, EmptyMutation, EmptySubscription)
        .register_type::<InterfaceA>() // `InterfaceA` is not directly referenced, so manual registration is required.
        .finish();
    let query = r#"{
            myObj {
               ... on InterfaceA {
                valueA
              }
              ... on InterfaceB {
                valueB
              }
              ... on MyObj {
                valueC
              }
              valueB2: valueB
            }
        }"#;
    assert_eq!(
        schema.execute(query).await.into_result().unwrap().data,
        value!({
            "myObj": {
                "valueA": 1,
                "valueB": 2,
                "valueB2": 2,
                "valueC": 3,
            }
        })
    );
}

#[async_std::test]
pub async fn test_multiple_objects_in_multiple_interfaces() {
    struct MyObjOne;

    #[Object]
    impl MyObjOne {
        async fn value_a(&self) -> i32 {
            1
        }

        async fn value_b(&self) -> i32 {
            2
        }

        async fn value_c(&self) -> i32 {
            3
        }
    }

    struct MyObjTwo;

    #[Object]
    impl MyObjTwo {
        async fn value_a(&self) -> i32 {
            1
        }
    }

    #[derive(Interface)]
    #[graphql(field(name = "value_a", type = "i32"))]
    enum InterfaceA {
        MyObjOne(MyObjOne),
        MyObjTwo(MyObjTwo),
    }

    #[derive(Interface)]
    #[graphql(field(name = "value_b", type = "i32"))]
    enum InterfaceB {
        MyObjOne(MyObjOne),
    }

    struct Query;

    #[Object]
    impl Query {
        async fn my_obj(&self) -> Vec<InterfaceA> {
            vec![MyObjOne.into(), MyObjTwo.into()]
        }
    }

    let schema = Schema::build(Query, EmptyMutation, EmptySubscription)
        .register_type::<InterfaceB>() // `InterfaceB` is not directly referenced, so manual registration is required.
        .finish();
    let query = r#"{
             myObj {
                ... on InterfaceA {
                 valueA
               }
               ... on InterfaceB {
                 valueB
               }
               ... on MyObjOne {
                 valueC
               }
               valueA2: valueA
             }
         }"#;
    assert_eq!(
        schema.execute(query).await.into_result().unwrap().data,
        value!({
            "myObj": [{
                "valueA": 1,
                "valueA2": 1,
                "valueB": 2,
                "valueC": 3,
            }, {
                "valueA": 1,
                "valueA2": 1,
            }]
        })
    );
}

#[async_std::test]
pub async fn test_interface_field_result() {
    struct MyObj;

    #[Object]
    impl MyObj {
        async fn value(&self) -> FieldResult<i32> {
            Ok(10)
        }
    }

    #[derive(Interface)]
    #[graphql(field(name = "value", type = "i32"))]
    enum Node {
        MyObj(MyObj),
    }

    struct Query;

    #[Object]
    impl Query {
        async fn node(&self) -> Node {
            MyObj.into()
        }
    }

    let query = r#"{
            node {
                value
            }
        }"#;
    let schema = Schema::new(Query, EmptyMutation, EmptySubscription);
    assert_eq!(
        schema.execute(query).await.into_result().unwrap().data,
        value!({
            "node": {
                "value": 10,
            }
        })
    );
}

#[async_std::test]
pub async fn test_interface_field_method() {
    struct A;

    #[Object]
    impl A {
        #[graphql(name = "created_at")]
        pub async fn created_at(&self) -> i32 {
            1
        }
    }

    struct B;

    #[Object]
    impl B {
        #[graphql(name = "created_at")]
        pub async fn created_at(&self) -> i32 {
            2
        }
    }

    #[derive(Interface)]
    #[graphql(field(name = "created_at", method = "created_at", type = "i32"))]
    enum MyInterface {
        A(A),
        B(B),
    }

    struct Query;

    #[Object]
    impl Query {
        async fn test(&self) -> MyInterface {
            A.into()
        }
    }

    let query = "{ test { created_at } }";
    let schema = Schema::new(Query, EmptyMutation, EmptySubscription);
    assert_eq!(
        schema.execute(query).await.into_result().unwrap().data,
        value!({
            "test": {
                "created_at": 1,
            }
        })
    );
}

#[async_std::test]
pub async fn test_interface_implement_other_interface() {
    #[derive(Interface)]
    #[graphql(field(name = "id", type = "ID"))]
    pub enum Entity {
        Company(Company),
        Organization(Organization),
    }

    #[derive(Interface)]
    #[graphql(field(name = "id", type = "ID"))]
    pub enum Node {
        Entity(Entity),
    }

    pub struct Company {}

    #[Object]
    impl Company {
        pub async fn id(&self) -> ID {
            "88".into()
        }
    }

    pub struct Organization {}

    #[Object]
    impl Organization {
        pub async fn id(&self) -> ID {
            "99".into()
        }
    }

    struct Query;

    #[Object]
    impl Query {
        async fn company(&self) -> Node {
            Entity::Company(Company {}).into()
        }

        async fn organization(&self) -> Node {
            Entity::Organization(Organization {}).into()
        }
    }

    let query = r#"
        {
            company { id }
            organization { id }
        }
    "#;
    let schema = Schema::new(Query, EmptyMutation, EmptySubscription);
    assert_eq!(
        schema.execute(query).await.into_result().unwrap().data,
        value!({
            "company": {
                "id": "88",
            },
            "organization": {
                "id": "99",
            }
        })
    );
}

#[async_std::test]
pub async fn test_issue_330() {
    #[derive(Interface)]
    #[graphql(field(
        desc = "The code represented as a number.",
        name = "number",
        type = "String"
    ))]
    pub enum Code {
        Barcode(Barcode),
        Qrcode(Qrcode),
    }

    pub struct Barcode(String);

    #[Object]
    impl Barcode {
        pub async fn number(&self) -> String {
            format!("barcode:{}", self.0)
        }
    }

    pub struct Qrcode(String);

    #[Object]
    impl Qrcode {
        pub async fn number(&self) -> String {
            format!("qrcode:{}", self.0)
        }
    }

    #[derive(Interface)]
    #[graphql(field(desc = "The article number.", name = "number", type = "Code"))]
    pub enum Article {
        Book(Book),
    }

    pub struct Book {
        code: String,
    }

    #[Object]
    impl Book {
        pub async fn number(&self) -> Barcode {
            Barcode(self.code.clone())
        }
    }

    struct Query;

    #[Object]
    impl Query {
        pub async fn book(&self) -> Article {
            Book {
                code: "123456".to_string(),
            }
            .into()
        }
    }

    let schema = Schema::new(Query, EmptyMutation, EmptySubscription);
    assert_eq!(
        schema
            .execute("{ book { number { number } } }")
            .await
            .into_result()
            .unwrap()
            .data,
        value!({
            "book": {
                "number": { "number": "barcode:123456" }
            }
        })
    );
}

#[async_std::test]
pub async fn test_interface_impl() {
    #[derive(SimpleObject)]
    struct MyObj1 {
        id: i32,
        title: String,
    }

    #[derive(SimpleObject)]
    struct MyObj2 {
        id: i32,
        name: String,
    }

    #[derive(Interface)]
    #[graphql(impl)]
    enum Node {
        MyObj1(MyObj1),
        MyObj2(MyObj2),
    }

    #[InterfaceImpl]
    impl Node {
        async fn id(&self) -> i32 {
            match self {
                Node::MyObj1(obj) => obj.id,
                Node::MyObj2(obj) => obj.id,
            }
        }

        async fn add(&self, n: i32) -> i32 {
            match self {
                Node::MyObj1(obj) => obj.id + n,
                Node::MyObj2(obj) => obj.id + n,
            }
        }
    }

    struct Query;

    #[Object]
    impl Query {
        async fn nodes(&self) -> Vec<Node> {
            vec![
                MyObj1 {
                    id: 1,
                    title: "haha".to_string(),
                }
                .into(),
                MyObj2 {
                    id: 2,
                    name: "hehe".to_string(),
                }
                .into(),
            ]
        }
    }

    let query = r#"{
            nodes {
                id
                id2: add(n: 5)
                ... on Node {
                    id3: id
                    id4: add(n: 5)
                    ... on MyObj1 {
                        __typename
                        title
                    }
                    ... on MyObj2 {
                        __typename
                        name
                    }
                }
            }
        }"#;
    let schema = Schema::new(Query, EmptyMutation, EmptySubscription);
    assert_eq!(
        schema.execute(query).await.into_result().unwrap().data,
        value!({
            "nodes": [
                {
                    "id": 1,
                    "id2": 6,
                    "id3": 1,
                    "id4": 6,
                    "__typename": "MyObj1",
                    "title": "haha",
                },
                {
                    "id": 2,
                    "id2": 7,
                    "id3": 2,
                    "id4": 7,
                    "__typename": "MyObj2",
                    "name": "hehe",
                }
            ]
        })
    );
}
