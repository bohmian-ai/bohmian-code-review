<script lang="ts">
  import { helper } from './util';
  let count = $state(0);
  function greet(name: string) {
    return helper(name);
  }
</script>

<h1 onclick={greet('x')}>{count}</h1>
<Modal title="hi" />
<button onclick={() => count++}>+</button>
