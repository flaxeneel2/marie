<script lang="ts">
  let { onClose } = $props<{ onClose: () => void }>();
</script>

<div class="interactive-badge">
  <span class="interactive-glow"></span>
  <span class="interactive-label">INTERACTIVE</span>
  <button class="interactive-close" onclick={onClose} title="Exit interactive mode">×</button>
</div>

<style>
  .interactive-badge {
    position: fixed;
    top: 16px;
    right: 16px;
    z-index: 9999;
    display: flex;
    align-items: center;
    gap: 10px;
    background: rgba(9, 10, 15, 0.95);
    border: 1px solid #e5a93b;
    border-radius: 20px;
    padding: 6px 12px 6px 16px;
    pointer-events: auto;
    box-shadow: 0 4px 20px rgba(0, 0, 0, 0.7), 0 0 10px rgba(229, 169, 59, 0.2);
    animation: badge-slide-in 0.25s cubic-bezier(0.16, 1, 0.3, 1) forwards;
  }

  .interactive-glow {
    width: 6px;
    height: 6px;
    border-radius: 50%;
    background: #e5a93b;
    box-shadow: 0 0 8px #e5a93b;
    animation: badge-pulse 1.5s infinite;
  }

  @keyframes badge-pulse {
    0% { opacity: 0.5; }
    50% { opacity: 1; }
    100% { opacity: 0.5; }
  }

  .interactive-label {
    font-family: 'Inter', 'Segoe UI', Arial, sans-serif;
    font-size: 10px;
    font-weight: 800;
    letter-spacing: 0.12em;
    color: #e5a93b;
    text-transform: uppercase;
  }

  .interactive-close {
    background: rgba(255, 255, 255, 0.03);
    border: 1px solid rgba(255, 255, 255, 0.1);
    border-radius: 50%;
    color: #94a3b8;
    cursor: pointer;
    font-size: 14px;
    line-height: 1;
    width: 22px;
    height: 22px;
    display: flex;
    align-items: center;
    justify-content: center;
    padding: 0;
    transition: all 0.15s ease;
  }

  .interactive-close:hover {
    border-color: #e5a93b;
    color: #e5a93b;
    background: rgba(229, 169, 59, 0.08);
  }

  @keyframes badge-slide-in {
    from {
      transform: translateY(-20px);
      opacity: 0;
    }
    to {
      transform: translateY(0);
      opacity: 1;
    }
  }
</style>
