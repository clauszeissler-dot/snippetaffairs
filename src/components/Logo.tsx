/**
 * KI-AffAIrs-Monogramm (KA mit Gipfel-Dreieck).
 *
 * Übernommen aus dem offiziellen Logopaket, Variante „RGB color flach"
 * (`Bild Assets/LOGOs/RGB_SVG/KI_AffAIrs_Logo_RGB_color_flach.svg`). Enthalten
 * ist nur der Bildteil bis y≈227 — die Wortmarke „AFFAIRS" darunter entfällt,
 * weil daneben ohnehin „SnippetAffAIrs" steht.
 *
 * Die Formen werden nicht nachgezeichnet, sondern 1:1 referenziert. Farben
 * kommen aus den Marken-Tokens und lassen sich per Prop überschreiben.
 */
interface Props {
  size?: number;
  /** Grundfarbe des Monogramms. */
  color?: string;
  /** Farbe des Gipfel-Dreiecks. */
  accent?: string;
  className?: string;
}

export default function Logo({
  size = 30,
  color = "var(--ki-logo-petrol)",
  accent = "var(--ki-logo-orange)",
  className,
}: Props) {
  return (
    <svg
      className={className}
      width={size}
      height={(size * 226.772) / 285.283}
      viewBox="1.7 0 283.583 226.772"
      xmlns="http://www.w3.org/2000/svg"
      role="img"
      aria-label="KI AffAIrs"
      focusable="false"
    >
      <polygon fill={accent} points="150.432 189.355 112.188 140.405 69.636 189.355 150.432 189.355" />
      <polygon fill={color} points="228.59 0 228.59 0 228.59 169.733 273.138 226.771 285.283 226.771 285.283 0 285.283 0 228.59 0" />
      <polygon fill={color} points="25.886 226.771 58.393 226.771 58.393 189.377 223.016 0 223.016 0 147.897 0 1.7 168.181 1.7 174.804 1.7 226.771 1.7 226.771 25.886 226.771" />
      <polygon fill={color} points="117.697 134.117 190.087 226.772 262.031 226.772 262.031 226.772 155.587 90.53 117.697 134.117" />
      <polygon fill={color} points="1.7 155.219 58.393 90.001 58.393 0 1.7 0 1.7 0 1.7 155.219" />
    </svg>
  );
}
