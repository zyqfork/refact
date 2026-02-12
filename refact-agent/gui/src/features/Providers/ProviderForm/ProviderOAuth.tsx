import React, { useState } from "react";
import { Button, Flex, Text, TextField } from "@radix-ui/themes";
import {
  useOauthStartMutation,
  useOauthExchangeMutation,
  useOauthLogoutMutation,
} from "../../../services/refact";

type ProviderOAuthProps = {
  providerName: string;
  oauthConnected: boolean;
  authStatus: string;
};

export const ProviderOAuth: React.FC<ProviderOAuthProps> = ({
  providerName,
  oauthConnected,
  authStatus,
}) => {
  const [oauthStart] = useOauthStartMutation();
  const [oauthExchange] = useOauthExchangeMutation();
  const [oauthLogout] = useOauthLogoutMutation();

  const [sessionId, setSessionId] = useState<string | null>(null);
  const [authorizeUrl, setAuthorizeUrl] = useState<string | null>(null);
  const [code, setCode] = useState("");
  const [error, setError] = useState<string | null>(null);
  const [isLoading, setIsLoading] = useState(false);

  const handleStartOAuth = async () => {
    setError(null);
    setIsLoading(true);
    try {
      const result = await oauthStart({ providerName, mode: "max" }).unwrap();
      setSessionId(result.session_id);
      setAuthorizeUrl(result.authorize_url);
      window.open(result.authorize_url, "_blank");
    } catch (e) {
      setError(e instanceof Error ? e.message : "Failed to start OAuth");
    } finally {
      setIsLoading(false);
    }
  };

  const handleExchangeCode = async () => {
    if (!sessionId || !code.trim()) return;
    setError(null);
    setIsLoading(true);
    try {
      await oauthExchange({
        providerName,
        session_id: sessionId,
        code: code.trim(),
      }).unwrap();
      setSessionId(null);
      setAuthorizeUrl(null);
      setCode("");
    } catch (e) {
      setError(e instanceof Error ? e.message : "Failed to exchange code");
    } finally {
      setIsLoading(false);
    }
  };

  const handleLogout = async () => {
    setError(null);
    setIsLoading(true);
    try {
      await oauthLogout({ providerName }).unwrap();
      setSessionId(null);
      setAuthorizeUrl(null);
      setCode("");
    } catch (e) {
      setError(e instanceof Error ? e.message : "Failed to logout");
    } finally {
      setIsLoading(false);
    }
  };

  if (oauthConnected) {
    return (
      <Flex direction="column" gap="2" p="3" style={{
        border: "1px solid var(--gray-6)",
        borderRadius: "var(--radius-2)",
      }}>
        <Flex align="center" justify="between">
          <Flex align="center" gap="2">
            <Text size="2" weight="medium" color="green">● Connected</Text>
            <Text size="1" color="gray">{authStatus}</Text>
          </Flex>
          <Button
            variant="ghost"
            color="red"
            size="1"
            disabled={isLoading}
            onClick={handleLogout}
          >
            Disconnect
          </Button>
        </Flex>
      </Flex>
    );
  }

  if (sessionId && authorizeUrl) {
    return (
      <Flex direction="column" gap="2" p="3" style={{
        border: "1px solid var(--gray-6)",
        borderRadius: "var(--radius-2)",
      }}>
        <Text size="2" weight="medium">Paste the authorization code</Text>
        <Text size="1" color="gray">
          A browser window should have opened. Log in and copy the code shown on the page.
        </Text>
        <Flex gap="2">
          <TextField.Root
            style={{ flex: 1 }}
            placeholder="Paste code here..."
            value={code}
            onChange={(e) => setCode(e.target.value)}
            onKeyDown={(e) => { if (e.key === "Enter") handleExchangeCode(); }}
          />
          <Button
            variant="solid"
            disabled={isLoading || !code.trim()}
            onClick={handleExchangeCode}
          >
            {isLoading ? "Connecting..." : "Connect"}
          </Button>
        </Flex>
        <Flex gap="2" align="center">
          <Text size="1" color="gray">
            Browser didn&apos;t open?{" "}
            <a
              href={authorizeUrl}
              target="_blank"
              rel="noopener noreferrer"
              style={{ color: "var(--accent-9)" }}
            >
              Click here
            </a>
          </Text>
          <Button
            variant="ghost"
            size="1"
            color="gray"
            onClick={() => {
              setSessionId(null);
              setAuthorizeUrl(null);
              setCode("");
            }}
          >
            Cancel
          </Button>
        </Flex>
        {error && <Text size="1" color="red">{error}</Text>}
      </Flex>
    );
  }

  return (
    <Flex direction="column" gap="2" p="3" style={{
      border: "1px solid var(--gray-6)",
      borderRadius: "var(--radius-2)",
    }}>
      <Flex align="center" justify="between">
        <Text size="2" weight="medium">Login with Anthropic</Text>
        <Button
          variant="solid"
          disabled={isLoading}
          onClick={handleStartOAuth}
        >
          {isLoading ? "Starting..." : "Login"}
        </Button>
      </Flex>
      {error && <Text size="1" color="red">{error}</Text>}
    </Flex>
  );
};
