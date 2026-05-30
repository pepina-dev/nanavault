<script lang="ts">
  // The "NanaVault" grandma mark, reconstructed from the reference design.
  let { size = 88 }: { size?: number } = $props();
  const ink = "#3a3a3a";
  const rose = "#ff385c";
</script>

<svg
  xmlns="http://www.w3.org/2000/svg"
  width={size}
  height={size}
  viewBox="0 0 120 120"
  fill="none"
  stroke-linecap="round"
  stroke-linejoin="round"
  role="img"
  aria-label="NanaVault"
>
  <!-- head -->
  <path
    d="M33 56 C31 30 42 15 60 15 C78 15 89 30 87 56 C89 78 76 94 60 94 C44 94 31 78 33 56 Z"
    stroke={ink}
    fill="#fff"
    stroke-width="3"
  />
  <!-- hair bun -->
  <circle cx="60" cy="17" r="9" stroke={ink} fill="#fff" stroke-width="3" />
  <!-- hair at the sides -->
  <path d="M40 38 C35 48 35 55 38 61" stroke={ink} fill="none" stroke-width="3" />
  <path d="M80 38 C85 48 85 55 82 61" stroke={ink} fill="none" stroke-width="3" />
  <!-- rose spectacles -->
  <g stroke={rose} fill="none" stroke-width="3">
    <circle cx="47" cy="60" r="10" />
    <circle cx="73" cy="60" r="10" />
    <path d="M57 60 Q60 57 63 60" />
    <path d="M37 58 L33 56" />
    <path d="M83 58 L87 56" />
  </g>
  <!-- eyes -->
  <circle cx="47" cy="61" r="2.4" fill={ink} />
  <circle cx="73" cy="61" r="2.4" fill={ink} />
  <!-- nose -->
  <path d="M60 65 L58.5 71" stroke={ink} fill="none" stroke-width="3" />
  <!-- smile -->
  <path d="M51 80 Q60 86 69 80" stroke={ink} fill="none" stroke-width="3" />
  <!-- rosy cheeks -->
  <circle cx="33" cy="67" r="2.2" fill={rose} />
  <circle cx="87" cy="67" r="2.2" fill={rose} />
</svg>
