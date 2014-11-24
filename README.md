alexandria-server
=================

The server component of Alexandria.

Setting up the DB
=================

1. Install PostgreSQL somewhere.
2. Run `createuser --no-superuser --no-createdb --no-createrole alexandria`.
3. Run `createdb --owner=alexandria alexandria`
4. Run `psql -d alexandria -U alexandria < src/init.sql`
