using System;
using System.ComponentModel;
using System.Reflection;
using System.Runtime.InteropServices;
using System.Windows.Forms;

namespace gma.System.Windows;

public class UserActivityHook
{
	[StructLayout(LayoutKind.Sequential)]
	private class POINT
	{
		public int x;

		public int y;
	}

	[StructLayout(LayoutKind.Sequential)]
	private class MouseHookStruct
	{
		public POINT pt;

		public int hwnd;

		public int wHitTestCode;

		public int dwExtraInfo;
	}

	[StructLayout(LayoutKind.Sequential)]
	private class MouseLLHookStruct
	{
		public POINT pt;

		public int mouseData;

		public int flags;

		public int time;

		public int dwExtraInfo;
	}

	[StructLayout(LayoutKind.Sequential)]
	private class KeyboardHookStruct
	{
		public int vkCode;

		public int scanCode;

		public int flags;

		public int time;

		public int dwExtraInfo;
	}

	private delegate int HookProc(int nCode, int wParam, IntPtr lParam);

	private const int WH_MOUSE_LL = 14;

	private const int WH_KEYBOARD_LL = 13;

	private const int WH_MOUSE = 7;

	private const int WH_KEYBOARD = 2;

	private const int WM_MOUSEMOVE = 512;

	private const int WM_LBUTTONDOWN = 513;

	private const int WM_RBUTTONDOWN = 516;

	private const int WM_MBUTTONDOWN = 519;

	private const int WM_LBUTTONUP = 514;

	private const int WM_RBUTTONUP = 517;

	private const int WM_MBUTTONUP = 520;

	private const int WM_LBUTTONDBLCLK = 515;

	private const int WM_RBUTTONDBLCLK = 518;

	private const int WM_MBUTTONDBLCLK = 521;

	private const int WM_MOUSEWHEEL = 522;

	private const int WM_KEYDOWN = 256;

	private const int WM_KEYUP = 257;

	private const int WM_SYSKEYDOWN = 260;

	private const int WM_SYSKEYUP = 261;

	private const byte VK_SHIFT = 16;

	private const byte VK_CAPITAL = 20;

	private const byte VK_NUMLOCK = 144;

	private int hMouseHook;

	private int hKeyboardHook;

	private static HookProc MouseHookProcedure;

	private static HookProc KeyboardHookProcedure;

	public event MouseEventHandler OnMouseActivity;

	public event KeyEventHandler KeyDown;

	public event KeyPressEventHandler KeyPress;

	public event KeyEventHandler KeyUp;

	[DllImport("user32.dll", CallingConvention = CallingConvention.StdCall, CharSet = CharSet.Auto, SetLastError = true)]
	private static extern int SetWindowsHookEx(int idHook, HookProc lpfn, IntPtr hMod, int dwThreadId);

	[DllImport("user32.dll", CallingConvention = CallingConvention.StdCall, CharSet = CharSet.Auto, SetLastError = true)]
	private static extern int UnhookWindowsHookEx(int idHook);

	[DllImport("user32.dll", CallingConvention = CallingConvention.StdCall, CharSet = CharSet.Auto)]
	private static extern int CallNextHookEx(int idHook, int nCode, int wParam, IntPtr lParam);

	[DllImport("user32")]
	private static extern int ToAscii(int uVirtKey, int uScanCode, byte[] lpbKeyState, byte[] lpwTransKey, int fuState);

	[DllImport("user32")]
	private static extern int GetKeyboardState(byte[] pbKeyState);

	[DllImport("user32.dll", CallingConvention = CallingConvention.StdCall, CharSet = CharSet.Auto)]
	private static extern short GetKeyState(int vKey);

	public UserActivityHook()
	{
		Start();
	}

	public UserActivityHook(bool InstallMouseHook, bool InstallKeyboardHook)
	{
		Start(InstallMouseHook, InstallKeyboardHook);
	}

	~UserActivityHook()
	{
		Stop(UninstallMouseHook: true, UninstallKeyboardHook: true, ThrowExceptions: false);
	}

	public void Start()
	{
		Start(InstallMouseHook: true, InstallKeyboardHook: true);
	}

	public void Start(bool InstallMouseHook, bool InstallKeyboardHook)
	{
		if (hMouseHook == 0 && InstallMouseHook)
		{
			MouseHookProcedure = MouseHookProc;
			hMouseHook = SetWindowsHookEx(14, MouseHookProcedure, Marshal.GetHINSTANCE(Assembly.GetExecutingAssembly().GetModules()[0]), 0);
			if (hMouseHook == 0)
			{
				int lastWin32Error = Marshal.GetLastWin32Error();
				Stop(UninstallMouseHook: true, UninstallKeyboardHook: false, ThrowExceptions: false);
				throw new Win32Exception(lastWin32Error);
			}
		}
		if (hKeyboardHook == 0 && InstallKeyboardHook)
		{
			KeyboardHookProcedure = KeyboardHookProc;
			hKeyboardHook = SetWindowsHookEx(13, KeyboardHookProcedure, Marshal.GetHINSTANCE(Assembly.GetExecutingAssembly().GetModules()[0]), 0);
			if (hKeyboardHook == 0)
			{
				int lastWin32Error2 = Marshal.GetLastWin32Error();
				Stop(UninstallMouseHook: false, UninstallKeyboardHook: true, ThrowExceptions: false);
				throw new Win32Exception(lastWin32Error2);
			}
		}
	}

	public void Stop()
	{
		Stop(UninstallMouseHook: true, UninstallKeyboardHook: true, ThrowExceptions: true);
	}

	public void Stop(bool UninstallMouseHook, bool UninstallKeyboardHook, bool ThrowExceptions)
	{
		if (hMouseHook != 0 && UninstallMouseHook)
		{
			int num = UnhookWindowsHookEx(hMouseHook);
			hMouseHook = 0;
			if (num == 0 && ThrowExceptions)
			{
				throw new Win32Exception(Marshal.GetLastWin32Error());
			}
		}
		if (hKeyboardHook != 0 && UninstallKeyboardHook)
		{
			int num2 = UnhookWindowsHookEx(hKeyboardHook);
			hKeyboardHook = 0;
			if (num2 == 0 && ThrowExceptions)
			{
				throw new Win32Exception(Marshal.GetLastWin32Error());
			}
		}
	}

	private int MouseHookProc(int nCode, int wParam, IntPtr lParam)
	{
		if (nCode >= 0 && this.OnMouseActivity != null)
		{
			MouseLLHookStruct mouseLLHookStruct = (MouseLLHookStruct)Marshal.PtrToStructure(lParam, typeof(MouseLLHookStruct));
			MouseButtons mouseButtons = MouseButtons.None;
			short delta = 0;
			switch (wParam)
			{
			case 513:
				mouseButtons = MouseButtons.Left;
				break;
			case 516:
				mouseButtons = MouseButtons.Right;
				break;
			case 522:
				delta = (short)((mouseLLHookStruct.mouseData >> 16) & 0xFFFF);
				break;
			}
			int clicks = 0;
			if (mouseButtons != MouseButtons.None)
			{
				clicks = ((wParam != 515 && wParam != 518) ? 1 : 2);
			}
			this.OnMouseActivity(this, new MouseEventArgs(mouseButtons, clicks, mouseLLHookStruct.pt.x, mouseLLHookStruct.pt.y, delta));
		}
		return CallNextHookEx(hMouseHook, nCode, wParam, lParam);
	}

	private int KeyboardHookProc(int nCode, int wParam, IntPtr lParam)
	{
		bool flag = false;
		if (nCode >= 0 && (this.KeyDown != null || this.KeyUp != null || this.KeyPress != null))
		{
			KeyboardHookStruct keyboardHookStruct = (KeyboardHookStruct)Marshal.PtrToStructure(lParam, typeof(KeyboardHookStruct));
			if (this.KeyDown != null && (wParam == 256 || wParam == 260))
			{
				KeyEventArgs e = new KeyEventArgs((Keys)keyboardHookStruct.vkCode);
				this.KeyDown(this, e);
				flag = flag || e.Handled;
			}
			if (this.KeyPress != null && wParam == 256)
			{
				bool flag2 = (GetKeyState(16) & 0x80) == 128;
				bool flag3 = GetKeyState(20) != 0;
				byte[] array = new byte[256];
				GetKeyboardState(array);
				byte[] array2 = new byte[2];
				if (ToAscii(keyboardHookStruct.vkCode, keyboardHookStruct.scanCode, array, array2, keyboardHookStruct.flags) == 1)
				{
					char c = (char)array2[0];
					if ((flag3 ^ flag2) && char.IsLetter(c))
					{
						c = char.ToUpper(c);
					}
					KeyPressEventArgs e2 = new KeyPressEventArgs(c);
					this.KeyPress(this, e2);
					flag = flag || e2.Handled;
				}
			}
			if (this.KeyUp != null && (wParam == 257 || wParam == 261))
			{
				KeyEventArgs e3 = new KeyEventArgs((Keys)keyboardHookStruct.vkCode);
				this.KeyUp(this, e3);
				flag = flag || e3.Handled;
			}
		}
		return flag ? 1 : CallNextHookEx(hKeyboardHook, nCode, wParam, lParam);
	}
}
