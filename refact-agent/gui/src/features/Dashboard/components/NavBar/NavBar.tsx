import React, { useCallback } from "react";
import { Text } from "@radix-ui/themes";
import {
  BarChartIcon,
  MixerHorizontalIcon,
  GearIcon,
  LightningBoltIcon,
  CubeIcon,
} from "@radix-ui/react-icons";
import { useAppDispatch } from "../../../../hooks";
import { push, type Page } from "../../../Pages/pagesSlice";
import styles from "./NavBar.module.css";

type NavItem = {
  icon: React.ReactNode;
  label: string;
  page: Page;
};

const ICON_SIZE = 15;

const NAV_ITEMS: NavItem[] = [
  { icon: <BarChartIcon width={ICON_SIZE} height={ICON_SIZE} />, label: "Stats", page: { name: "stats dashboard" } },
  { icon: <MixerHorizontalIcon width={ICON_SIZE} height={ICON_SIZE} />, label: "Integrations", page: { name: "integrations page" } },
  { icon: <GearIcon width={ICON_SIZE} height={ICON_SIZE} />, label: "Providers", page: { name: "providers page" } },
  { icon: <LightningBoltIcon width={ICON_SIZE} height={ICON_SIZE} />, label: "Knowledge", page: { name: "knowledge graph" } },
  { icon: <CubeIcon width={ICON_SIZE} height={ICON_SIZE} />, label: "Marketplace", page: { name: "mcp marketplace" } },
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
        <button
          key={item.page.name}
          type="button"
          className={styles.navButton}
          onClick={() => handleClick(item.page)}
        >
          <span className={styles.icon}>{item.icon}</span>
          <Text size="1" className={styles.label}>{item.label}</Text>
        </button>
      ))}
    </nav>
  );
};
