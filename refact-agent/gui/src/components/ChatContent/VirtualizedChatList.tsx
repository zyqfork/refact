import React, { useCallback, useRef, useState, useMemo } from "react";
import { Virtuoso, VirtuosoHandle } from "react-virtuoso";
import { Flex, Container, Box } from "@radix-ui/themes";
import { ScrollToBottomButton } from "../ScrollArea/ScrollToBottomButton";
import styles from "./ChatContent.module.css";

export type VirtualizedChatListProps<T extends { key: string }> = {
  items: T[];
  renderItem: (item: T) => React.ReactNode;
  initialScrollIndex?: number;
  footer?: React.ReactNode;
  isStreaming?: boolean;
};

export function VirtualizedChatList<T extends { key: string }>({
  items,
  renderItem,
  initialScrollIndex,
  footer,
  isStreaming = false,
}: VirtualizedChatListProps<T>) {
  const virtuosoRef = useRef<VirtuosoHandle>(null);
  const [showFollowButton, setShowFollowButton] = useState(false);
  const autoFollowRef = useRef(true);
  const userScrolledUpRef = useRef(false);
  const lastScrollTopRef = useRef(0);
  const wasScrollingDownRef = useRef(false);

  const handleAtBottomChange = useCallback((bottom: boolean) => {
    if (bottom && userScrolledUpRef.current) {
      // Only re-arm auto-follow if the user actively scrolled down to
      // reach the bottom.  Content height changes (tool cards
      // expanding/collapsing, task_done rendering) can passively move the
      // bottom threshold to the user — that must NOT re-arm follow.
      const activeScroll = wasScrollingDownRef.current;
      wasScrollingDownRef.current = false;
      userScrolledUpRef.current = false;
      if (activeScroll) {
        autoFollowRef.current = true;
      }
    }
    setShowFollowButton(!bottom && userScrolledUpRef.current);
  }, []);

  const handleFollowClick = useCallback(() => {
    autoFollowRef.current = true;
    userScrolledUpRef.current = false;
    setShowFollowButton(false);
    virtuosoRef.current?.scrollToIndex({
      index: items.length - 1,
      align: "end",
      behavior: "smooth",
    });
  }, [items.length]);

  const followOutput = useCallback(
    (isAtBottom: boolean) => {
      if (userScrolledUpRef.current) return false;
      if (isAtBottom && autoFollowRef.current) {
        return isStreaming ? "auto" : "smooth";
      }
      return false;
    },
    [isStreaming],
  );

  const computeItemKey = useCallback((_index: number, item: T) => item.key, []);

  const itemContent = useCallback(
    (_index: number, item: T) => <Container>{renderItem(item)}</Container>,
    [renderItem],
  );

  const Scroller = useMemo(() => {
    const ScrollerComponent = React.forwardRef<
      HTMLDivElement,
      React.HTMLAttributes<HTMLDivElement>
      // eslint-disable-next-line react/prop-types
    >(function VirtuosoScroller(
      { children, style, onWheel, onScroll, ...props },
      ref,
    ) {
      const handleWheel: React.WheelEventHandler<HTMLDivElement> = (event) => {
        if (event.deltaY < 0) {
          autoFollowRef.current = false;
          userScrolledUpRef.current = true;
          setShowFollowButton(true);
        }
        onWheel?.(event);
      };

      const handleScroll: React.UIEventHandler<HTMLDivElement> = (event) => {
        const nextScrollTop = event.currentTarget.scrollTop;
        if (nextScrollTop + 1 < lastScrollTopRef.current) {
          autoFollowRef.current = false;
          userScrolledUpRef.current = true;
          wasScrollingDownRef.current = false;
          setShowFollowButton(true);
        } else if (nextScrollTop > lastScrollTopRef.current + 1) {
          wasScrollingDownRef.current = true;
        }
        lastScrollTopRef.current = nextScrollTop;
        onScroll?.(event);
      };

      return (
        <div
          ref={ref}
          style={{
            ...style,
            overflowY: "auto",
            overflowX: "hidden",
          }}
          className={styles.virtuosoScroller}
          {...props}
          onWheel={handleWheel}
          onScroll={handleScroll}
        >
          {children}
        </div>
      );
    });
    return ScrollerComponent;
  }, []);

  const List = useMemo(() => {
    const ListComponent = React.forwardRef<
      HTMLDivElement,
      React.HTMLAttributes<HTMLDivElement>
      // eslint-disable-next-line react/prop-types
    >(function VirtuosoList({ children, style, ...props }, ref) {
      return (
        <Flex
          ref={ref}
          direction="column"
          className={styles.content}
          p="2"
          gap="1"
          style={style}
          {...props}
        >
          {children}
        </Flex>
      );
    });
    return ListComponent;
  }, []);

  const Footer = useCallback(
    () => (
      <>
        {footer}
        <Box style={{ height: 80 }} />
      </>
    ),
    [footer],
  );

  const components = useMemo(
    () => ({ Scroller, List, Footer }),
    [Scroller, List, Footer],
  );

  const viewportPadding = useMemo(
    () =>
      isStreaming
        ? { top: 800, bottom: 1200 }
        : { top: 1600, bottom: 2200 },
    [isStreaming],
  );

  return (
    <Box style={{ flexGrow: 1, height: "100%", position: "relative" }}>
      <Virtuoso
        ref={virtuosoRef}
        data={items}
        computeItemKey={computeItemKey}
        itemContent={itemContent}
        components={components}
        atBottomStateChange={handleAtBottomChange}
        followOutput={followOutput}
        initialTopMostItemIndex={
          initialScrollIndex !== undefined
            ? { index: initialScrollIndex, align: "end" }
            : undefined
        }
        atBottomThreshold={20}
        increaseViewportBy={viewportPadding}
      />
      {showFollowButton && <ScrollToBottomButton onClick={handleFollowClick} />}
    </Box>
  );
}

VirtualizedChatList.displayName = "VirtualizedChatList";
