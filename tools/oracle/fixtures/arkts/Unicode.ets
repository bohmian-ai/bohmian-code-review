@Component
struct Ünïcode {
  @State message: string = 'héllo wörld 😀';
  @State 値: number = 42;

  build() {
    Column({ space: 8 }) {
      Text(this.message)
        .fontSize(18)
      Text(this.値.toString())
    }
  }
}
