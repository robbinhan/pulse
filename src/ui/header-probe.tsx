import { Text, View, mount, signal } from "nsh";
import { Menu, MenuItem } from "nsh/base";

/** The card header's menu and badge, without a profile, so a test can press them. */
function Probe() {
  const menuOpen = signal(false);
  const unread = signal(7);
  return (
    <View class="flex-row items-center gap-3 p-6">
      <If when={unread() > 0}>
        <View
          onPress={() => unread.set(0)}
          accessibilityLabel="把这个格子标为已读"
          class="h-8 min-w-8 items-center justify-center rounded-full bg-primary px-2"
        >
          <Text class="text-xs font-semibold text-primary-foreground">{unread()}</Text>
        </View>
      </If>
      <Menu
        open={menuOpen()}
        onOpenChange={(open) => menuOpen.set(open)}
        placement="bottom"
        align="end"
        offset={4}
        popupClass="min-w-36 rounded-[12px] border border-border bg-popover p-1 shadow-md"
        trigger={
          <View accessibilityLabel="账号菜单" class="h-8 w-8 items-center justify-center rounded-[8px]">
            <Text class="text-base text-muted-foreground">⋯</Text>
          </View>
        }
      >
        <MenuItem onSelect={() => menuOpen.set(false)} class="rounded-[8px] px-3 py-2">
          <Text class="text-sm text-popover-foreground">编辑账号</Text>
        </MenuItem>
      </Menu>
    </View>
  );
}

mount(Probe);
