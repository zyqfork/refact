import React, { useCallback } from "react";
import { Text, Tooltip } from "@radix-ui/themes";
import { useAppDispatch } from "../../../../hooks";
import { push, type Page } from "../../../Pages/pagesSlice";
import styles from "./NavBar.module.css";

type NavItem = {
  icon: string;
  label: string;
  page: Page;
};

const NAV_ITEMS: NavItem[] = [
  { icon: "📊", label: "Stats", page: { name: "stats dashboard" } },
  { icon: "🔌", label: "Integrations", page: { name: "integrations page" } },
  { icon: "⚙", label: "Providers", page: { name: "providers page" } },
  { icon: "🧠", label: "Knowledge", page: { name: "knowledge graph" } },
  { icon: "🛒", label: "Marketplace", page: { name: "mcp marketplace" } },
];

export const NavBar: React.FC = () => {
  const dispatch = useAppDispatch();

  const handleClick = useCallback(
    (page: Page) => {
      dispatch(push(page));
    },
    [dispatch],
  );

  return (
    <nav className={styles.nav}>
      {NAV_ITEMS.map((item) => (
        <Tooltip key={item.page.name} content={item.label}>
          <button
            type="button"
            className={styles.navButton}
            onClick={() => handleClick(item.page)}
          >
            <span className={styles.icon}>{item.icon}</span>
            <Text size="1" className={styles.label}>{item.label}</Text>
          </button>
        </Tooltip>
      ))}
    </nav>
  );
};
