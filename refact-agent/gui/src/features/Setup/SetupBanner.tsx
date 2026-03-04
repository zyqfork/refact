import React, { useCallback, useEffect, useState } from "react";
import { Button, Flex, Text } from "@radix-ui/themes";
import { Cross1Icon } from "@radix-ui/react-icons";
import { CalloutFromTop } from "../../components/Callout/Callout";
import { useGetSetupStatusQuery } from "../../services/refact/setupStatus";
import { useAppDispatch } from "../../hooks/useAppDispatch";
import { createChatWithId } from "../Chat/Thread/actions";
import { push } from "../Pages/pagesSlice";
import styles from "./SetupBanner.module.css";

const DISMISS_KEY = "refact-setup-banner-dismissed";

function isDismissed(projectRoot: string | null | undefined): boolean {
  if (!projectRoot) return false;
  try {
    const dismissed = JSON.parse(
      localStorage.getItem(DISMISS_KEY) ?? "{}",
    ) as Record<string, boolean>;
    return dismissed[projectRoot] === true;
  } catch {
    return false;
  }
}

function setDismissed(projectRoot: string | null | undefined) {
  if (!projectRoot) return;
  try {
    const dismissed = JSON.parse(
      localStorage.getItem(DISMISS_KEY) ?? "{}",
    ) as Record<string, boolean>;
    dismissed[projectRoot] = true;
    localStorage.setItem(DISMISS_KEY, JSON.stringify(dismissed));
  } catch {
    // ignore
  }
}

export const SetupBanner: React.FC = () => {
  const dispatch = useAppDispatch();
  const { data, isError } = useGetSetupStatusQuery(undefined, {
    refetchOnMountOrArgChange: true,
  });

  const [localDismissed, setLocalDismissed] = useState(false);

  const projectRoot = data?.detail?.project_root;

  useEffect(() => setLocalDismissed(false), [projectRoot]);

  const openSetupChat = useCallback(() => {
    dispatch(createChatWithId({ id: globalThis.crypto.randomUUID(), mode: "setup" }));
    dispatch(push({ name: "chat" }));
  }, [dispatch]);

  const handleDismiss = useCallback(() => {
    setDismissed(projectRoot);
    setLocalDismissed(true);
  }, [projectRoot]);

  if (isError || !data || data.configured) return null;
  if (localDismissed || isDismissed(projectRoot)) return null;

  return (
    <CalloutFromTop>
      <Flex direction={{ initial: "column", sm: "row" }} gap="3" align="center">
        <Text size="2" className={styles.text}>
          This project hasn't been set up for Refact yet. Run setup to generate
          guidelines, integrations, and toolbox commands.
        </Text>
        <Flex gap="2" align="center" style={{ flexShrink: 0 }}>
          <Button size="2" onClick={openSetupChat}>
            Run Setup
          </Button>
          <Button
            size="1"
            variant="ghost"
            color="gray"
            onClick={handleDismiss}
            aria-label="Dismiss"
          >
            <Cross1Icon width={12} height={12} />
          </Button>
        </Flex>
      </Flex>
    </CalloutFromTop>
  );
};
