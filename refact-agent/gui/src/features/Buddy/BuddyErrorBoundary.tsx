import React from "react";
import { Flex, Text } from "@radix-ui/themes";
import { reportBuddyFrontendError } from "./reportBuddyFrontendError";
import styles from "./BuddyErrorBoundary.module.css";

type Props = {
  children: React.ReactNode;
};

type State = {
  failed: boolean;
};

export class BuddyErrorBoundary extends React.Component<Props, State> {
  override state: State = {
    failed: false,
  };

  static getDerivedStateFromError(): State {
    return { failed: true };
  }

  override componentDidCatch(error: Error, errorInfo: React.ErrorInfo): void {
    const details = errorInfo.componentStack
      ? `${error.stack || error.message}\n\nComponent stack:\n${
          errorInfo.componentStack
        }`
      : error;

    void reportBuddyFrontendError({
      source: "react_error_boundary",
      error: details,
      sourceFile: "frontend/react_error_boundary",
      toolName: "react_error_boundary",
    });
  }

  override render(): React.ReactNode {
    if (this.state.failed) {
      return (
        <Flex align="center" justify="center" className={styles.root}>
          <div className={styles.card}>
            <Text size="3" weight="bold">
              The app hit a frontend error.
            </Text>
            <Text size="2" color="gray">
              Buddy recorded it for investigation. Reload the view if it stays
              blank.
            </Text>
          </div>
        </Flex>
      );
    }

    return this.props.children;
  }
}
