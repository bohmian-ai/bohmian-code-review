@Component
struct Broken {
  @State items: string[] = [];

  build() {
    Column() {
      ForEach(this.items, (item: string) => {
        Text(item
      })
    }
  }
}

function {
