@Component
struct GoodList {
  @State items: string[] = ['a', 'b'];

  build() {
    Column() {
      ForEach(this.items, (item: string) => {
        Text(item).fontSize(16)
      }, (item: string) => item)
    }
  }
}

@Extend(Text)
function brokenExtend(this: Text) {
  .fontSize(20
  .fontWeight(FontWeight.Bold)
}
