import clsx from 'clsx';
import { IconProps, sizeMap } from './icon.type'

export default function Icon({
  icon: IconComponent,
  size = 'md',
  color = 'currentcolor',
  variant,
  className,
  ...props
}: IconProps) {

  const resolvedSize =
    typeof size === 'string' && size in sizeMap
      ? sizeMap[size]
      : size;

  const isDecorative = !(props['aria-label'] || props['aria-labelledby']);
  const resolvedColor = variant ? `var(--color-${variant}-400)` : color;

  return (
    <IconComponent
      className={clsx("icon", className)}
      width={resolvedSize}
      height={resolvedSize}
      color={resolvedColor}
      aria-hidden={isDecorative ? 'true' : undefined}
      role={isDecorative ? undefined : 'img'}
      focusable="false"
      {...props}
    />
  )
}
