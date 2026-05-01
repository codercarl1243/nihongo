'use client'

import clsx from 'clsx'
import type { ButtonProps, ButtonMouseEvent } from './type'
import Spinner from '../spinner'
import Icon from '../icon'
import { handleKeyPress } from '../../lib/keyboardHandler'
import { Block } from '../../primitives'

/**
 * A base, accessible button component that supports loading states and 
 * safely manages click behavior and logging through a custom hook.
 *  
 * This component uses aria-disabled instead of the native disabled attribute
 * to maintain discoverability and tab order. The button remains focusable
 * and discoverable to assistive technology while in this non-functional state.
 *
 * @component
 * @param {BaseButtonProps} props - The props for the Button component.
 * @param {string} [props.className] - Additional class names to append.
 * @param {React.ReactNode} props.children - The button label or content.
 * @param {function} [props.onClick] - Optional click handler.
 * @param {'button' | 'submit' | 'reset'} [props.type='button'] - The button type.
 * @param {boolean} [props.disabled=false] - Whether the button is disabled.
 * @param {boolean} [props.isLoading=false] - Whether the button is in a loading state.
 * @returns {JSX.Element} The rendered Button component.
 *
 * @example
 * // Loading state with spinner
 * <Button isLoading onClick={handleSubmit}>Submit</Button>
 * 
 * // Disabled but discoverable
 * <Button disabled onClick={handleSave}>Save</Button>
 * 
 * // With variant styling
 * <Button variant="primary" variantAppearance="filled">
 *   Click Me
 * </Button>
 */
export default function Button({
  className,
  children,
  onClick,
  type = 'button',
  disabled = false,
  isLoading = false,
  showSpinner = false,
  icon,
  onKeyDown,
  onKeyUp,
  ...props
}: ButtonProps) {

  /**
   * Handles click events.
   *
   * - Prevents default form submission when loading or disabled.
   * - Stops propagation to avoid parent click triggers.
   * - Delegates to useButton's click handler otherwise.
   */
  function onClickHandler(event: ButtonMouseEvent) {
    if (isLoading || disabled) {
      /**  
       * Using both is correct here since a disabled/loading button should do nothing and 
       * not trigger parent handlers.
       * */
      event.stopPropagation()     // Prevent triggering parent click handlers
      event.preventDefault()      // Prevent form submission and/or default click actions
      return;
    }
    void onClick?.(event)
  }

  function handleKeyDown(event: React.KeyboardEvent<HTMLButtonElement>) {
    handleKeyPress(event, {
      Enter: () => (event.currentTarget.dataset.pressed = 'true'),
    })
    onKeyDown?.(event)
  }

  function handleKeyUp(event: React.KeyboardEvent<HTMLButtonElement>) {
    handleKeyPress(event, {
      Enter: () => delete event.currentTarget.dataset.pressed,
    })
    onKeyUp?.(event)
  }
  return (
    <Block
      as="button"
      className={clsx(
        className,
        { 'button-w-icon': icon },
        { 'button-w-spinner': showSpinner },
        'button')}
      onClick={onClickHandler}
      aria-disabled={isLoading || disabled}
      data-loading={isLoading}
      type={type}
      onKeyDown={handleKeyDown}
      onKeyUp={handleKeyUp}
      data-testid="base-button"
      paint="all"
      {...props}
    >
      {icon && <Icon icon={icon} />}
      <span className='button__content'>{children}</span>
      {(showSpinner && isLoading) && <Spinner />}
    </Block>
  )
}
