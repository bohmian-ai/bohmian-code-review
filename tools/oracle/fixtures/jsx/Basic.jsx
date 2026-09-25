import { helper } from './util';

export function App(props) {
  return <div onClick={() => helper(props)}>hello</div>;
}
