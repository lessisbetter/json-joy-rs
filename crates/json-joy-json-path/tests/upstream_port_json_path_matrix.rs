use json_joy_json_path::{JsonPathEval, JsonPathParser};
use serde_json::{json, Value};

fn bookstore() -> Value {
    json!({
        "store": {
            "book": [
                {"category": "reference", "author": "Nigel Rees", "title": "Sayings of the Century", "price": 8.95},
                {"category": "fiction", "author": "Evelyn Waugh", "title": "Sword of Honour", "price": 12.99},
                {"category": "fiction", "author": "Herman Melville", "title": "Moby Dick", "isbn": "0-553-21311-3", "price": 8.99},
                {"category": "fiction", "author": "J. R. R. Tolkien", "title": "The Lord of the Rings", "isbn": "0-395-19395-8", "price": 22.99}
            ],
            "bicycle": {"color": "red", "price": 399}
        }
    })
}

fn eval_values(path: &str, data: &Value) -> Vec<Value> {
    let parsed =
        JsonPathParser::parse(path).unwrap_or_else(|e| panic!("parse failed for '{path}': {e}"));
    JsonPathEval::eval(&parsed, data)
        .into_iter()
        .cloned()
        .collect()
}

#[test]
fn upstream_bookstore_core_query_matrix() {
    let data = bookstore();

    let authors = eval_values("$.store.book[*].author", &data);
    assert_eq!(
        authors,
        vec![
            json!("Nigel Rees"),
            json!("Evelyn Waugh"),
            json!("Herman Melville"),
            json!("J. R. R. Tolkien"),
        ]
    );

    let all_authors = eval_values("$..author", &data);
    assert_eq!(all_authors.len(), 4);

    let store_children = eval_values("$.store[*]", &data);
    assert_eq!(store_children.len(), 2);

    let all_prices = eval_values("$..price", &data);
    assert_eq!(all_prices.len(), 5);
    assert!(all_prices.contains(&json!(8.95)));
    assert!(all_prices.contains(&json!(12.99)));
    assert!(all_prices.contains(&json!(8.99)));
    assert!(all_prices.contains(&json!(22.99)));
    assert!(all_prices.contains(&json!(399)));
}

#[test]
fn upstream_bookstore_index_and_slice_matrix() {
    let data = bookstore();

    let third_book = eval_values("$..book[2]", &data);
    assert_eq!(third_book.len(), 1);
    assert_eq!(third_book[0]["title"], json!("Moby Dick"));

    let last_book = eval_values("$..book[-1]", &data);
    assert_eq!(last_book.len(), 1);
    assert_eq!(last_book[0]["title"], json!("The Lord of the Rings"));

    let first_two_union = eval_values("$..book[0,1]", &data);
    assert_eq!(first_two_union.len(), 2);
    assert_eq!(first_two_union[0]["title"], json!("Sayings of the Century"));
    assert_eq!(first_two_union[1]["title"], json!("Sword of Honour"));

    let first_two_slice = eval_values("$..book[:2]", &data);
    assert_eq!(first_two_slice.len(), 2);
    assert_eq!(first_two_slice[0]["title"], json!("Sayings of the Century"));
    assert_eq!(first_two_slice[1]["title"], json!("Sword of Honour"));
}

#[test]
fn upstream_bookstore_filter_matrix() {
    let data = bookstore();

    let with_isbn = eval_values("$..book[?@.isbn]", &data);
    assert_eq!(with_isbn.len(), 2);
    assert_eq!(with_isbn[0]["title"], json!("Moby Dick"));
    assert_eq!(with_isbn[1]["title"], json!("The Lord of the Rings"));

    let cheap_books = eval_values("$..book[?@.price < 10]", &data);
    assert_eq!(cheap_books.len(), 2);
    assert_eq!(cheap_books[0]["title"], json!("Sayings of the Century"));
    assert_eq!(cheap_books[1]["title"], json!("Moby Dick"));
}

#[test]
fn upstream_recursive_descent_invalid_matrix() {
    assert!(JsonPathParser::parse("$..").is_err());
}
