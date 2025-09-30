CREATE TABLE user (
    id SERIAL PRIMARY KEY,
    user_principal BINARY NOT NULL
);

-- CREATE TABLE user_setup_callback (
--     id SERIAL PRIMARY KEY,
--     user_setup_id INT NOT NULL
--     -- callback_canister_id BINARY NOT NULL,
--     -- callback_func TEXT NOT NULL
-- );

CREATE TABLE server_setup (
    id SERIAL PRIMARY KEY,
    user_id INT NOT NULL,
    server_prefix TEXT NOT NULL
);

CREATE TABLE header (
    id SERIAL PRIMARY KEY,
    server_setup_id INT NOT NULL,
    header_name TEXT NOT NULL,
    header_value TEXT NOT NULL
);
