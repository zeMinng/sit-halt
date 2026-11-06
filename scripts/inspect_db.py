import os
import sqlite3

db = os.path.join(os.environ["APPDATA"], "com.sithalt.app", "sithalt.db")
conn = sqlite3.connect(db)
tables = [r[0] for r in conn.execute("select name from sqlite_master where type='table' order by name")]
print("tables:", tables)
print("migrations:", conn.execute("select count(*) from _sqlx_migrations").fetchone()[0])
print("plans:", conn.execute("select count(*) from reminder_plans").fetchone()[0])
print("activities:", conn.execute("select count(*) from activities").fetchone()[0])
print("settings:", conn.execute("select count(*) from app_settings").fetchone()[0])
print("events:", conn.execute("select event_type, created_at from app_events order by id").fetchall())
print(
    "plan rows:",
    conn.execute(
        "select name, work_seconds, break_seconds from reminder_plans order by sort_order"
    ).fetchall(),
)
