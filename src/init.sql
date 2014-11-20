DROP TABLE IF EXISTS books CASCADE;
CREATE TABLE books (
    id SERIAL PRIMARY KEY,
    name VARCHAR NOT NULL,
    description VARCHAR NOT NULL,
    isbn VARCHAR UNIQUE NOT NULL,
    cover_image VARCHAR NOT NULL,
    available SMALLINT NOT NULL,
    quantity SMALLINT NOT NULL,
    active_date TIMESTAMP NOT NULL,
    permission SMALLINT NOT NULL,
    book_history INTEGER REFERENCES history(id)
);

DROP TABLE IF EXISTS history CASCADE;
CREATE TABLE history (
    id SERIAL PRIMARY KEY,
    user INTEGER REFERENCES users(id),
    date TIMESTAMP
);

DROP TABLE IF EXISTS users CASCADE;
CREATE TABLE users (
    id SERIAL PRIMARY KEY,
    email VARCHAR NOT NULL,
    username VARCHAR NOT NULL,
    student_id VARCHAR NOT NULL,
    permission SMALLINT NOT NULL
);
