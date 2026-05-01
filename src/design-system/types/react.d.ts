declare module 'react' {

  /**
   * Adds support for defining components that can render as different HTML elements.
   * Similar to Radix UI's polymorphic utility types.
   */
  export type AsProp<T extends React.ElementType> = {
    as?: T;
  };

  /**
   * Determines which props to omit when combining props for polymorphic components.
   */
  export type PropsToOmit<T extends React.ElementType, Props> =
    keyof (AsProp<T> & Props);

  /**
 * Props for a polymorphic component.
 * Combines the props of the specified element type with additional custom props,
 * omitting any conflicting props.
 * @example
 * ```tsx
 * type MyComponentProps<T extends React.ElementType> = PolymorphicProps<
 *   T,
 *   { customProp: string }
 * >;
 * ```
 */
  export type PolymorphicProps<
    T extends React.ElementType,
    Props = {}
  > = Props &
    AsProp<T> &
    Omit<React.ComponentPropsWithRef<T>, PropsToOmit<T, Props>>;

  /**
   * Polymorphic component type:
   * Allows `<Component as="a">` with proper type inference.
   * 
   * @example
   * ```tsx
   * const MyComponent: PolymorphicComponent<"button", { customProp: string }> = ({ as, customProp, ...props }) => {
   *   const Component = as || "button";
   *   return <Component {...props} />;
   * };
   * ```
   */
  export interface PolymorphicComponent<
    DefaultElement extends React.ElementType,
    Props = {}
  > {
    <T extends React.ElementType = DefaultElement>(
      props: PolymorphicProps<T, Props>
    ): React.ReactElement | null;

    displayName?: string;
  }
}

export { };