import { router } from '@kit.ArkUI';

@Component
export struct TodoItem {
  @Prop title: string;
  @State completed: boolean = false;

  build() {
    Row({ space: 8 }) {
      Text(this.title)
        .fontSize(16)
        .opacity(this.completed ? 0.5 : 1)
      Button(this.completed ? 'Undo' : 'Done')
        .onClick(() => {
          this.completed = !this.completed;
        })
    }
    .width('100%')
    .padding(12)
  }
}

@Entry
@Component
struct TodoList {
  @State items: TodoItem[] = [];
  @State count: number = 0;

  @Builder
  itemBuilder(item: TodoItem) {
    Text(item.title).fontSize(14)
  }

  build() {
    Column({ space: 8 }) {
      ForEach(this.items, (item: TodoItem) => {
        this.itemBuilder(item)
      }, (item: TodoItem) => item.title)
      Button('Add')
        .onClick(() => {
          this.count++
        })
    }
    .width('100%')
  }
}

@Builder
function globalBuilder() {
  Text('global').fontSize(20)
}

@Extend(Text)
function bold(this: Text) {
  .fontWeight(FontWeight.Bold)
}

@Styles
function cardStyle() {
  .backgroundColor('#FFFFFF')
  .borderRadius(8)
}
