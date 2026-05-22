<script lang="ts">
  type NavItem = 'inventory' | 'foundry' | 'dev';
  let { activeNav = $bindable() } = $props<{ activeNav: NavItem }>();
</script>

<nav class="sidebar">
  <div class="sidebar-logo">
    <span class="logo-text">marie</span>
    <span class="logo-accent">HUD</span>
  </div>

  <div class="nav-group">
    <button
      class="nav-item"
      class:active={activeNav === 'inventory'}
      onclick={() => (activeNav = 'inventory')}
    >
      <span class="nav-icon">⬡</span>
      <span class="nav-label">Inventory</span>
    </button>
    <button
      class="nav-item"
      class:active={activeNav === 'foundry'}
      onclick={() => (activeNav = 'foundry')}
    >
      <span class="nav-icon">⚒</span>
      <span class="nav-label">Foundry</span>
    </button>
  </div>

  <div class="nav-group nav-bottom">
    <button
      class="nav-item nav-dev"
      class:active={activeNav === 'dev'}
      onclick={() => (activeNav = 'dev')}
    >
      <span class="nav-icon">⚙</span>
      <span class="nav-label">Dev Tools</span>
    </button>
  </div>
</nav>

<style>
  .sidebar {
    width: 200px;
    min-width: 200px;
    background: linear-gradient(180deg, #090a0f 0%, #06070a 100%);
    border-right: 1px solid rgba(56, 189, 248, 0.12);
    display: flex;
    flex-direction: column;
    padding: 0;
    position: relative;
    box-shadow: 4px 0 24px rgba(0, 0, 0, 0.4);
  }

  .sidebar::after {
    content: '';
    position: absolute;
    top: 0;
    right: 0;
    bottom: 0;
    width: 1px;
    background: linear-gradient(180deg, rgba(56, 189, 248, 0) 0%, rgba(56, 189, 248, 0.3) 50%, rgba(56, 189, 248, 0) 100%);
    pointer-events: none;
  }

  .sidebar-logo {
    padding: 24px 20px;
    border-bottom: 1px solid rgba(56, 189, 248, 0.08);
    display: flex;
    align-items: center;
    gap: 6px;
  }

  .logo-text {
    font-family: 'Inter', sans-serif;
    font-size: 1.4rem;
    font-weight: 800;
    color: #ffffff;
    letter-spacing: 0.15em;
    text-shadow: 0 0 10px rgba(255, 255, 255, 0.1);
  }

  .logo-accent {
    font-family: 'Inter', sans-serif;
    font-size: 0.75rem;
    font-weight: 700;
    color: #e5a93b;
    letter-spacing: 0.1em;
    border: 1px solid rgba(229, 169, 59, 0.4);
    border-radius: 4px;
    padding: 1px 4px;
    text-transform: uppercase;
  }

  .nav-group {
    display: flex;
    flex-direction: column;
    padding: 16px 8px;
    gap: 4px;
  }

  .nav-bottom {
    margin-top: auto;
    border-top: 1px solid rgba(56, 189, 248, 0.08);
  }

  .nav-item {
    display: flex;
    align-items: center;
    gap: 12px;
    padding: 12px 16px;
    background: transparent;
    border: none;
    border-radius: 6px;
    color: #8f9cae;
    font-size: 13px;
    font-weight: 600;
    cursor: pointer;
    text-align: left;
    transition: all 0.2s cubic-bezier(0.4, 0, 0.2, 1);
    position: relative;
    overflow: hidden;
  }

  .nav-item::before {
    content: '';
    position: absolute;
    left: 0;
    top: 50%;
    transform: translateY(-50%) scaleY(0);
    height: 60%;
    width: 3px;
    background: #e5a93b;
    border-radius: 0 2px 2px 0;
    transition: transform 0.2s ease;
  }

  .nav-item:hover {
    background: rgba(56, 189, 248, 0.05);
    color: #e2e8f0;
    padding-left: 20px;
  }

  .nav-item:hover::before {
    transform: translateY(-50%) scaleY(0.5);
  }

  .nav-item.active {
    background: rgba(229, 169, 59, 0.08);
    color: #e5a93b;
    font-weight: 700;
    box-shadow: inset 1px 0 0 rgba(229, 169, 59, 0.2);
  }

  .nav-item.active::before {
    transform: translateY(-50%) scaleY(1);
  }

  .nav-dev {
    color: #64748b;
  }

  .nav-dev.active {
    color: #e5a93b;
  }

  .nav-icon {
    font-size: 15px;
    width: 16px;
    text-align: center;
    display: inline-block;
    transition: transform 0.2s ease;
  }

  .nav-item:hover .nav-icon {
    transform: scale(1.15);
  }

  .nav-item.active .nav-icon {
    text-shadow: 0 0 8px rgba(229, 169, 59, 0.5);
  }
</style>
