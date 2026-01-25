<script lang="ts">
  import '../app.css';
</script>

<div class="app-root" data-testid="app-ready">
  <slot />
</div>

<style>
  .app-root {
    display: flex;
    height: 100vh;
    width: 100vw;
    overflow: hidden;
  }
</style>
