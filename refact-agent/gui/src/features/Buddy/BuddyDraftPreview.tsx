import React from "react";
import { Callout, Text, Flex } from "@radix-ui/themes";
import { InfoCircledIcon } from "@radix-ui/react-icons";
import type { BuddyDraft } from "./types";

type Props = {
  draft: BuddyDraft;
};

export const BuddyDraftPreview: React.FC<Props> = ({ draft }) => {
  return (
    <Callout.Root color="blue" mb="2">
      <Callout.Icon>
        <InfoCircledIcon />
      </Callout.Icon>
      <Callout.Text>
        <Flex direction="column" gap="1">
          <Text size="2" weight="bold">
            Buddy Draft: {draft.title}
          </Text>
          {draft.explanation && <Text size="1">{draft.explanation}</Text>}
        </Flex>
      </Callout.Text>
    </Callout.Root>
  );
};
