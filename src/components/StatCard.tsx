import type { ReactNode } from "react";
import { Card } from "antd";

interface StatCardProps {
  title: string;
  value: ReactNode;
  hint?: ReactNode;
  tone?: "default" | "primary";
}

export function StatCard({ title, value, hint, tone = "default" }: StatCardProps) {
  return (
    <Card size="small" variant="borderless" className="stat-card">
      <div className="stat-card-title">{title}</div>
      <div className={`stat-card-value ${tone === "primary" ? "is-primary" : ""}`}>{value}</div>
      {hint ? <div className="stat-card-hint">{hint}</div> : null}
    </Card>
  );
}
