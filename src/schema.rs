// @generated automatically by Diesel CLI.

diesel::table! {
    links (id) {
        id -> Integer,
        src -> Text,
        target -> Text,
        created_at -> Timestamp,
        updated_at -> Timestamp,
    }
}
