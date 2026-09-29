/*
** Frozen source excerpt from SQLite 3.53.2 ext/fts5/fts5_unicode2.c.
**
** SQLite source ID:
** 2026-06-03 19:12:13 d6e03d8c777cfa2d35e3b60d8ec3e0187f3e9f99d8e2ee9cac695fd6fcdf1a24
** Upstream tag:
** https://github.com/sqlite/sqlite/blob/version-3.53.2/ext/fts5/fts5_unicode2.c
** Verified against the bundled amalgamation from libsqlite3-sys 0.38.2.
**
** SQLite dedicates this source to the public domain. This excerpt is kept as
** deterministic development input and is never compiled into the crate.
*/
static int sqlite3Fts5UnicodeIsdiacritic(int c){
  unsigned int mask0 = 0x08029FDF;
  unsigned int mask1 = 0x000361F8;
  if( c<768 || c>817 ) return 0;
  return (c < 768+32) ?
      (mask0 & ((unsigned int)1 << (c-768))) :
      (mask1 & ((unsigned int)1 << (c-768-32)));
}
