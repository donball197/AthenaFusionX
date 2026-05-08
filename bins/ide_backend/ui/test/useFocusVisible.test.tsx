/**
 * Copyright 2026 Donald Ball
 * Licensed under the Apache License, Version 2.0
 */

import {expect} from 'chai';
import React, {useRef} from 'react';
import {render, fireEvent, act} from '@testing-library/react';
import {renderHook} from '@testing-library/react-hooks';
import {useFocusVisible} from '../';

describe('useFocusVisible', () => {
  it('should return isFocusVisible: true when tab is pressed', () => {
    const {result} = renderHook(() => useFocusVisible());
    expect(result.current.isFocusVisible).to.be.false;

    act(() => {
      window.dispatchEvent(new KeyboardEvent('keydown', {key: 'Tab'}));
    });
    expect(result.current.isFocusVisible).to.be.true;
  });

  it('should return isFocusVisible: false when a mouse is used', () => {
    const {result} = renderHook(() => useFocusVisible());
    
    act(() => {
      window.dispatchEvent(new KeyboardEvent('keydown', {key: 'Tab'}));
    });
    expect(result.current.isFocusVisible).to.be.true;

    act(() => {
      window.dispatchEvent(new MouseEvent('mousedown'));
    });
    expect(result.current.isFocusVisible).to.be.false;
  });

  it('should use isFocusVisible from props if provided', () => {
    const {result, rerender} = renderHook((props) => useFocusVisible(props), {
      initialProps: {isFocusVisible: true}
    });
    expect(result.current.isFocusVisible).to.be.true;

    rerender({isFocusVisible: false});
    expect(result.current.isFocusVisible).to.be.false;
  });
});

describe('useFocusVisible on focus', () => {
  function FocusVisibleComponent({isFocusVisibleProp}: {isFocusVisibleProp?: boolean}) {
    const ref = useRef<HTMLButtonElement>(null);
    const {isFocusVisible} = useFocusVisible({isFocusVisible: isFocusVisibleProp});

    return (
      <button ref={ref} data-testid="button">
        {isFocusVisible ? 'Focus Visible' : 'Focus Hidden'}
      </button>
    );
  }

  it('should follow global isFocusVisible state', () => {
    const {getByTestId} = render(<FocusVisibleComponent isFocusVisibleProp={undefined} />);
    const button = getByTestId('button');

    expect(button.textContent).to.equal('Focus Hidden');

    act(() => {
      window.dispatchEvent(new KeyboardEvent('keydown', {key: 'Tab'}));
      button.focus();
    });
    
    expect(button.textContent).to.equal('Focus Visible');
  });
});
