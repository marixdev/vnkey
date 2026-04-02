using System.CodeDom.Compiler;
using System.ComponentModel;
using System.Diagnostics;
using System.Drawing;
using System.Globalization;
using System.Resources;
using System.Runtime.CompilerServices;

namespace VnKey.Properties;

[GeneratedCode("System.Resources.Tools.StronglyTypedResourceBuilder", "16.0.0.0")]
[DebuggerNonUserCode]
[CompilerGenerated]
public class Resources
{
	private static ResourceManager resourceMan;

	private static CultureInfo resourceCulture;

	[EditorBrowsable(EditorBrowsableState.Advanced)]
	public static ResourceManager ResourceManager
	{
		get
		{
			if (resourceMan == null)
			{
				ResourceManager resourceManager = new ResourceManager("VnKey.Properties.Resources", typeof(Resources).Assembly);
				resourceMan = resourceManager;
			}
			return resourceMan;
		}
	}

	[EditorBrowsable(EditorBrowsableState.Advanced)]
	public static CultureInfo Culture
	{
		get
		{
			return resourceCulture;
		}
		set
		{
			resourceCulture = value;
		}
	}

	public static string Chuoiketnoi => ResourceManager.GetString("Chuoiketnoi", resourceCulture);

	public static Icon icon16V
	{
		get
		{
			object obj = ResourceManager.GetObject("icon16V", resourceCulture);
			return (Icon)obj;
		}
	}

	public static Icon icon32E
	{
		get
		{
			object obj = ResourceManager.GetObject("icon32E", resourceCulture);
			return (Icon)obj;
		}
	}

	public static Icon icon32Help
	{
		get
		{
			object obj = ResourceManager.GetObject("icon32Help", resourceCulture);
			return (Icon)obj;
		}
	}

	public static Icon icon32V
	{
		get
		{
			object obj = ResourceManager.GetObject("icon32V", resourceCulture);
			return (Icon)obj;
		}
	}

	public static Icon iconCheck
	{
		get
		{
			object obj = ResourceManager.GetObject("iconCheck", resourceCulture);
			return (Icon)obj;
		}
	}

	public static Bitmap iconE
	{
		get
		{
			object obj = ResourceManager.GetObject("iconE", resourceCulture);
			return (Bitmap)obj;
		}
	}

	public static Bitmap iconHelp2
	{
		get
		{
			object obj = ResourceManager.GetObject("iconHelp2", resourceCulture);
			return (Bitmap)obj;
		}
	}

	public static Bitmap iconTasktbar
	{
		get
		{
			object obj = ResourceManager.GetObject("iconTasktbar", resourceCulture);
			return (Bitmap)obj;
		}
	}

	public static Bitmap iconThoat
	{
		get
		{
			object obj = ResourceManager.GetObject("iconThoat", resourceCulture);
			return (Bitmap)obj;
		}
	}

	public static Bitmap iconV
	{
		get
		{
			object obj = ResourceManager.GetObject("iconV", resourceCulture);
			return (Bitmap)obj;
		}
	}

	public static Bitmap tam2
	{
		get
		{
			object obj = ResourceManager.GetObject("tam2", resourceCulture);
			return (Bitmap)obj;
		}
	}

	public static Bitmap TpBank
	{
		get
		{
			object obj = ResourceManager.GetObject("TpBank", resourceCulture);
			return (Bitmap)obj;
		}
	}

	internal Resources()
	{
	}
}
