export const REEDY_DB_SCHEMA = `
CREATE TABLE IF NOT EXISTS reedy_book_meta (
  book_hash TEXT PRIMARY KEY,
  indexing_status TEXT NOT NULL CHECK(indexing_status IN ('not_indexed','indexing','indexed','failed','empty_index')),
  chunk_count INTEGER NOT NULL DEFAULT 0,
  embedding_model TEXT NOT NULL,
  embedding_dim INTEGER NOT NULL,
  indexed_at INTEGER,
  error TEXT
);

CREATE TABLE IF NOT EXISTS reedy_book_chunks (
  id TEXT PRIMARY KEY,
  book_hash TEXT NOT NULL,
  section_index INTEGER NOT NULL,
  chapter_title TEXT,
  start_cfi TEXT NOT NULL,
  end_cfi TEXT NOT NULL,
  position_index INTEGER NOT NULL,
  text TEXT NOT NULL,
  token_count INTEGER NOT NULL
);

CREATE INDEX IF NOT EXISTS idx_chunks_book_position ON reedy_book_chunks(book_hash, position_index);

CREATE VIRTUAL TABLE IF NOT EXISTS reedy_book_chunks_fts USING fts5(
  text,
  content=reedy_book_chunks,
  content_rowid=rowid,
  tokenize='unicode61'
);

CREATE TABLE IF NOT EXISTS reedy_book_chunk_embeddings (
  chunk_id TEXT PRIMARY KEY REFERENCES reedy_book_chunks(id) ON DELETE CASCADE,
  book_hash TEXT NOT NULL,
  embedding BLOB NOT NULL
);

CREATE INDEX IF NOT EXISTS idx_embeddings_book ON reedy_book_chunk_embeddings(book_hash);

CREATE TABLE IF NOT EXISTS reedy_memory (
  id TEXT PRIMARY KEY,
  scope TEXT NOT NULL CHECK(scope IN ('user','book','session')),
  scope_key TEXT NOT NULL,
  key TEXT NOT NULL,
  summary TEXT NOT NULL,
  source_message_id TEXT,
  updated_at INTEGER NOT NULL,
  UNIQUE(scope, scope_key, key)
);

CREATE INDEX IF NOT EXISTS idx_memory_scope ON reedy_memory(scope, scope_key, updated_at DESC);

CREATE TABLE IF NOT EXISTS reedy_memory_embeddings (
  memory_id TEXT PRIMARY KEY REFERENCES reedy_memory(id) ON DELETE CASCADE,
  embedding BLOB NOT NULL
);

CREATE TABLE IF NOT EXISTS reedy_skills (
  id TEXT PRIMARY KEY,
  name TEXT NOT NULL,
  description TEXT NOT NULL,
  instructions TEXT NOT NULL,
  tool_allowlist TEXT,
  builtin INTEGER NOT NULL DEFAULT 1,
  enabled INTEGER NOT NULL DEFAULT 1
);

CREATE TABLE IF NOT EXISTS reedy_metrics (
  id INTEGER PRIMARY KEY AUTOINCREMENT,
  ts INTEGER NOT NULL,
  event TEXT NOT NULL,
  book_hash TEXT,
  session_id TEXT,
  turn_id TEXT,
  message_id TEXT,
  app_version TEXT NOT NULL,
  schema_version INTEGER NOT NULL DEFAULT 1,
  payload TEXT
);

CREATE INDEX IF NOT EXISTS idx_metrics_ts ON reedy_metrics(ts DESC);
CREATE INDEX IF NOT EXISTS idx_metrics_session ON reedy_metrics(session_id, ts DESC);
`

export function applyMigrations(db: import('better-sqlite3').Database): void {
  db.pragma('journal_mode = WAL')
  db.pragma('foreign_keys = ON')
  db.pragma('synchronous = NORMAL')
  db.exec(REEDY_DB_SCHEMA)
}
