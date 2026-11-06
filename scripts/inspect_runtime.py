import json
import os
import sqlite3

db = os.path.join(os.environ["APPDATA"], "com.sithalt.app", "sithalt.db")
conn = sqlite3.connect(db)
print("sitting_sessions:", conn.execute(
    "select started_at, ended_at, duration_seconds, interrupted_by from sitting_sessions order by started_at"
).fetchall())
print("activity_sessions:", conn.execute(
    "select activity_name, completion, duration_seconds, source from activity_sessions"
).fetchall())
checkpoint = conn.execute("select value from app_settings where key = 'engineCheckpoint'").fetchone()
print("checkpoint:", json.loads(checkpoint[0]) if checkpoint else None)
print("app_settings:", conn.execute("select key, value from app_settings order by key").fetchall())
