import { Core } from "theysay/native";
import { Text } from "nsh";
import { Button } from "nsh/base";
import { applyParsed } from "./model";

export function StarButton(props: { platform: string; id: string; on: boolean }) {
  function toggle() {
    applyParsed(JSON.parse(Core.toggleStar(props.platform, props.id)));
  }
  return (
    <Button onPress={toggle} accessibilityLabel={props.on ? "取消星标" : "星标"} class="active:opacity-70">
      <Text class={props.on ? "text-xs font-semibold text-primary" : "text-xs font-medium text-muted-foreground"}>
        {props.on ? "已星标" : "星标"}
      </Text>
    </Button>
  );
}
