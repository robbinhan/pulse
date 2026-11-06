import { Core } from "theysay/native";
import { Scroll, Text, View, mount, signal, type YNode } from "nsh";

const items = signal(["1", "2", "3", "4", "5", "6", "7", "8", "9", "10", "11", "12"]);
const offsetText = signal("0");
const opened = new Set<string>();

function Probe() {
  let scroller!: YNode;
  let offsetY = 0;
  let contentH = 0;
  return (
    <View class="flex-1 bg-background" style={{ height: 844 }}>
      <Scroll
        testID="feed"
        ref={scroller}
        zoomable={false}
        scrollable
        style={{ height: 220 }}
        onScroll={(event) => {
          const offset = event.offset.y;
          const view = event.size.height;
          const content = event.contentSize.height;
          const next = Core.scrollOffset(offsetY, offset, view, contentH, content);
          contentH = content;
          offsetY = offset;
          if (next >= 0 && scroller && Math.abs(next - offset) > 1) {
            offsetY = next;
            scroller.scrollTo({ y: next, animated: false });
          }
          offsetText.set(String(Math.round(offsetY)));
        }}
      >
        {items().map((id) => (
          <View style={{ height: 100 }}>
            <Text>{`帖子 ${id}`}</Text>
          </View>
        ))}
      </Scroll>
      <View
        testID="add"
        onPress={() => items.set(items().concat(String(items().length + 1)))}
        role="button"
      >
        <Text>加帖</Text>
      </View>
      <Text testID="offset">{offsetText()}</Text>
      <View
        testID="open"
        onPress={() => {
          if (!scroller || opened.has("feed")) return;
          opened.add("feed");
          scroller.scrollToEnd({ animated: false });
        }}
        role="button"
      >
        <Text>打开到底</Text>
      </View>
    </View>
  );
}

mount(Probe);
