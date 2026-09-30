CREATE TABLE products (
    id UUID PRIMARY KEY,
    name TEXT NOT NULL,
    description TEXT,
    stock BIGINT NOT NULL,
    weight_kg DOUBLE PRECISION NOT NULL,
    price BIGINT NOT NULL,
    available BOOLEAN NOT NULL,
    size TEXT NOT NULL,
    colors JSONB NOT NULL,
    opens_at TIME,
    launch_at TIMESTAMP,
    released_on DATE,
    created_at TIMESTAMPTZ,
    updated_at TIMESTAMPTZ
);
