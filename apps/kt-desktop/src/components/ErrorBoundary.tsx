import { Component, type ReactNode } from "react";
import { AlertTriangleIcon } from "./Icons";

interface Props {
  children: ReactNode;
}

interface State {
  hasError: boolean;
  error: Error | null;
}

export class ErrorBoundary extends Component<Props, State> {
  constructor(props: Props) {
    super(props);
    this.state = { hasError: false, error: null };
  }

  static getDerivedStateFromError(error: Error): State {
    return { hasError: true, error };
  }

  componentDidCatch(error: Error, errorInfo: React.ErrorInfo) {
    console.error("ErrorBoundary caught an error:", error, errorInfo);
  }

  handleReload = () => {
    window.location.reload();
  };

  handleReset = () => {
    this.setState({ hasError: false, error: null });
  };

  render() {
    if (this.state.hasError) {
      return (
        <div className="h-screen flex items-center justify-center bg-bg-void text-text-primary">
          <div className="max-w-md text-center p-8">
            {/* Error icon */}
            <div className="w-16 h-16 mx-auto mb-6 rounded-full bg-terracotta/10 flex items-center justify-center">
              <AlertTriangleIcon className="w-8 h-8 text-terracotta" />
            </div>

            {/* Error message */}
            <h1 className="text-xl font-semibold mb-2 text-text-primary">
              Something went wrong
            </h1>
            <p className="text-text-muted mb-6">
              The application encountered an unexpected error. You can try
              reloading the page or resetting the application state.
            </p>

            {/* Error details (collapsed by default) */}
            {this.state.error && (
              <details className="mb-6 text-left">
                <summary className="cursor-pointer text-sm text-text-ghost hover:text-text-muted">
                  Technical details
                </summary>
                <pre className="mt-2 p-3 bg-bg-surface rounded-zen text-xs text-terracotta overflow-auto max-h-32">
                  {this.state.error.message}
                  {this.state.error.stack && (
                    <>
                      {"\n\n"}
                      {this.state.error.stack}
                    </>
                  )}
                </pre>
              </details>
            )}

            {/* Action buttons */}
            <div className="flex gap-3 justify-center">
              <button
                onClick={this.handleReset}
                className="px-4 py-2 text-sm font-medium rounded-zen bg-bg-elevated hover:bg-bg-hover border border-border text-text-secondary transition-colors"
              >
                Try Again
              </button>
              <button
                onClick={this.handleReload}
                className="px-4 py-2 text-sm font-medium rounded-zen bg-mauve hover:bg-mauve-mid text-white transition-colors"
              >
                Reload App
              </button>
            </div>
          </div>
        </div>
      );
    }

    return this.props.children;
  }
}
