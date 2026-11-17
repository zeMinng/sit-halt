import { Component, type ErrorInfo, type ReactNode } from "react";
import { Button, Result, Space, Typography } from "antd";
import { useNavigate } from "react-router-dom";

interface ErrorBoundaryProps {
  children: ReactNode;
}

interface ErrorBoundaryState {
  hasError: boolean;
  message: string;
}

function Fallback({ message, onReset }: { message: string; onReset: () => void }) {
  const navigate = useNavigate();
  return (
    <div
      style={{
        display: "flex",
        flexDirection: "column",
        alignItems: "center",
        justifyContent: "center",
        gap: 12,
        padding: 48,
      }}
    >
      <Result
        status="warning"
        title="页面出了点问题"
        subTitle={
          message ? (
            <Typography.Text type="secondary">{message}</Typography.Text>
          ) : (
            <Typography.Text type="secondary">当前页面渲染失败，可重试或返回首页。</Typography.Text>
          )
        }
        extra={
          <Space>
            <Button onClick={onReset}>重试</Button>
            <Button type="primary" onClick={() => navigate("/dashboard")}>
              返回首页
            </Button>
          </Space>
        }
      />
    </div>
  );
}

/**
 * 全局错误边界：lazy 页面或组件在渲染期抛错时，
 * 不再一直卡在 Suspense 的“加载页面…”，而是显示可操作的兜底错误页。
 */
export class ErrorBoundary extends Component<ErrorBoundaryProps, ErrorBoundaryState> {
  state: ErrorBoundaryState = { hasError: false, message: "" };

  static getDerivedStateFromError(error: unknown): ErrorBoundaryState {
    return {
      hasError: true,
      message: error instanceof Error ? error.message : String(error),
    };
  }

  componentDidCatch(error: unknown, info: ErrorInfo) {
    // 保留到控制台便于排查；界面上只展示精简信息
    console.error("[ErrorBoundary]", error, info.componentStack);
  }

  private reset = () => {
    this.setState({ hasError: false, message: "" });
  };

  render() {
    if (this.state.hasError) {
      return <Fallback message={this.state.message} onReset={this.reset} />;
    }
    return this.props.children;
  }
}
