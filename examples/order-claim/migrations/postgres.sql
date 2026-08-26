CREATE TABLE IF NOT EXISTS orders (
  id UUID PRIMARY KEY,
  status TEXT NOT NULL CHECK (status IN ('available', 'claimed')),
  claimed_by TEXT,
  claimed_at TIMESTAMPTZ,
  created_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE TABLE IF NOT EXISTS inventory_reservations (
  order_id UUID PRIMARY KEY REFERENCES orders(id),
  user_id TEXT NOT NULL,
  created_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

INSERT INTO orders (id, status) VALUES ('11111111-1111-4111-8111-111111111111', 'available') ON CONFLICT DO NOTHING;
