CREATE TABLE books (
    id SERIAL PRIMARY KEY,
    name VARCHAR NOT NULL,
    description VARCHAR NOT NULL,
    isbn VARCHAR UNIQUE NOT NULL,
    cover_image BYTEA NOT NULL,
    available SMALLINT NOT NULL,
    quantity SMALLINT NOT NULL,
    active_date TIMESTAMP NOT NULL,
    permission SMALLINA NOT NULLT
);

CREATE TABLE history (
    id SERIAL PRIMARY KEY,
    book INTEGER REFERENCES books (id),
    date TIMESTAMP,
    action SMALLINT
);

CREATE TABLE users (
    id SERIAL PRIMARY KEY,
    email VARCHAR NOT NULL,
    student_id VARCHAR NOT NULL
);
