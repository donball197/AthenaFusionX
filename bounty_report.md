# 🛡️ Nexus Singularity Bug Bounty Report

## 🚨 Vulnerability: SQL_INJECTION
- **Severity:** High
- **File:** `./nexus_audit_target/www/blindsqli.php` (Sink lines 32-48)

### Description
User input from $_GET["user"] is directly concatenated into the SQL query string $q = "Select * from users where username = '".$user."'" and passed to mysqli_query without sanitization or prepared statements, allowing attacker-controlled SQL injection.

### Vulnerable Code
```php
		if (!mysqli_query($con,$q))
	{
		//die('Error: ' . mysqli_error($con));
	}else{
		$result = mysqli_query($con,$q);
		

		$row = mysqli_fetch_array($result);
		

		if ($row){
		$_SESSION["username"] = $row[1];
		$_SESSION["name"] = $row[3];
		$_SESSION["descr"] = $row[4];
		
}
	}
```

### Patch (DEFERRED: multi-site taint -- `$q` built via concatenation at lines 30-30, not patched)

---

## 🚨 Vulnerability: SQL_INJECTION
- **Severity:** High
- **File:** `./nexus_audit_target/www/login1.php` (Sink lines 78-120)

### Description
User-controlled $_REQUEST['uid'] and $_REQUEST['password'] are concatenated directly into the SQL string $q without escaping or prepared statements and the resulting query is executed via mysqli_query, allowing classic SQL injection via username.

### Vulnerable Code
```php
		if (!mysqli_query($con,$q))
	{
		echo 'Error: ' . mysqli_error($con);
	}else{
	
	$result = mysqli_query($con,$q);

	// if (!$result) {
 //    		printf("%s\n", mysqli_error($con));
 //    		echo "error";
	// }

if (mysqli_warning_count($con)) { 
   $e = mysqli_get_warnings($con); 
   if ($e){
   do { 
       echo "Warning: $e->errno: $e->message\n"; 
   } while ($e->next()); }
} 

	echo "<br /><br />";
	$row = mysqli_fetch_array($result);

	
	if ($row){
	//$_SESSION["id"] = $row[0];
	$_SESSION["username"] = $row[1];
	$_SESSION["name"] = $row[3];
	//ob_clean();
	
	if ($_SESSION['next']=="searchproducts.php"){
	header('Location: searchproducts.php');
	}elseif ($_SESSION['next']=="blindsqli.php"){
		header('Location: blindsqli.php?user='.$_SESSION["username"]);
		}elseif ($_SESSION['next']=="os_sqli.php"){
			header('Location: os_sqli.php?user='.$_SESSION["username"]);
			}
	
	}
	else{
		echo "<font style=\"color:#FF0000\">Invalid password!</font\>";
	}
}
```

### Patch (DEFERRED: multi-site taint -- `$q` built via concatenation at lines 67-67, not patched)

---

## 🚨 Vulnerability: SQL_INJECTION
- **Severity:** High
- **File:** `./nexus_audit_target/www/login2.php` (Sink lines 80-116)

### Description
$username from $_REQUEST['uid'] and $pass from $_REQUEST['password'] are concatenated directly into $q without escaping or prepared statements. The constructed query is passed to mysqli_query, allowing attacker-controlled input to alter SQL logic. Debug output also leaks the raw query.

### Vulnerable Code
```php
		if (!mysqli_query($con,$q))
	{
		echo 'Error: ' . mysqli_error($con);
	}else{
	
	$result = mysqli_query($con,$q);

	if (mysqli_warning_count($con)) { 
   $e = mysqli_get_warnings($con); 
   if ($e){
   do { 
       echo "Warning: $e->errno: $e->message\n"; 
   } while ($e->next()); }
} 

	echo "<br /><br />";
	$row = mysqli_fetch_array($result);

	
	if ($row){
	//$_SESSION["id"] = $row[0];
	$_SESSION["username"] = $row[1];
	$_SESSION["name"] = $row[3];
	//ob_clean();
	
	if ($_SESSION['next']=="searchproducts.php"){
		header('Location: searchproducts.php');
		}elseif ($_SESSION['next']=="blindsqli.php"){
			header('Location: blindsqli.php?user='.$_SESSION["username"]);
			}elseif ($_SESSION['next']=="os_sqli.php"){
				header('Location: os_sqli.php?user='.$_SESSION["username"]);
				}
		}
	else{
		echo "<font style=\"color:#FF0000\">Invalid password!</font\>";
	}
}
```

### Patch (DEFERRED: multi-site taint -- `$q` built via concatenation at lines 68-68, not patched)

---

## 🚨 Vulnerability: SQL_INJECTION
- **Severity:** High
- **File:** `./nexus_audit_target/www/os_sqli.php` (Sink lines 33-51)

### Description
$_GET['user'] is taken directly from untrusted request input and concatenated into SQL string $q = "Select * from users where username = '".$user."'" with no sanitization, escaping, or prepared statement. The query is executed via mysqli_multi_query which allows multiple statements, enabling classic SQL injection and OS interaction.

### Vulnerable Code
```php
		if (mysqli_multi_query($con,$q))
		{
		  do
			{
			// Store first result set
			if ($result=mysqli_store_result($con)) {
			  // Fetch one and one row
			  while ($row=mysqli_fetch_row($result))
				{
					$username = $row[1];
					$name = $row[3];
					$descr = $row[4];
				}
			  // Free result set
			  mysqli_free_result($result);
			  }
			}
		  while (mysqli_next_result($con));
		}
```

### Patch (DEFERRED: multi-site taint -- `$q` built via concatenation at lines 31-31, not patched)

---

## 🚨 Vulnerability: SQL_INJECTION
- **Severity:** High
- **File:** `./nexus_audit_target/www/register.php` (Sink lines 111-122)

### Description
User-controlled $_REQUEST['uid'], $_REQUEST['password'], $_REQUEST['name'], $_REQUEST['descr'] are concatenated directly into the INSERT SQL string $q and executed with mysqli_query without escaping, prepared statements, or validation. Untrusted input reaches the SQL sink, enabling classic SQL injection.

### Vulnerable Code
```php
	if (!mysqli_query($con,$q))
	{
		echo ('Error: ' . mysqli_error($con));
	}else{
	
	
	$_SESSION["username"] = $username;
	$_SESSION["fname"] = $fname;
	
	ob_clean();
	header('Location:searchproducts.php');
	}
```

### Patch (DEFERRED: multi-site taint -- `$q` built via concatenation at lines 98-98, not patched)

---

## 🚨 Vulnerability: SQL_INJECTION
- **Severity:** High
- **File:** `./nexus_audit_target/www/secondorder_home.php` (Sink lines 124-127)

### Description
User-supplied $_REQUEST values for username, password, name and descr are concatenated directly into an INSERT query string passed to mysqli_query without prepared statements or escaping, allowing SQL injection.

### Vulnerable Code
```php
	if (!mysqli_query($con,$q))
	{
		die('Error: ' . mysqli_error($con));
	}
```

### Patch (DEFERRED: multi-site taint -- `$q` built via concatenation at lines 122-122, not patched)

---

## 🚨 Vulnerability: SQL_INJECTION
- **Severity:** High
- **File:** `./nexus_audit_target/www/secondorder_register.php` (Sink lines 112-123)

### Description
Untrusted $_REQUEST data for uid/name/descr is concatenated into an INSERT statement after only str_replace("'",'""') escaping. No prepared statements or proper mysqli_real_escape_string are used, so user input reaches the SQL sink and can be manipulated for SQL injection.

### Vulnerable Code
```php
	if (!mysqli_query($con,$q))
	{
		echo('Error: ' . mysqli_error($con));
	}else{
	
	
	$_SESSION["id"] = $id;
	$_SESSION["fname"] = $fname;
	
	ob_clean();
	header('Location:secondorder_home.php');
	}
```

### Patch (DEFERRED: multi-site taint -- `$q` built via concatenation at lines 101-101, not patched)

---


# Final Audit Summary

- **Actionable findings:** 7
- **Verified fixed:** 0
- **Remediation failed:** 0
- **Already resolved (earlier patch in this run):** 0
- **Explicitly safe:** 0
- **Unknown (needs manual review):** 3
- **Deferred (multi-site taint, not patched):** 7
- **Triage errors (API):** 0

## Needs Manual Review

- `./nexus_audit_target/www/searchproducts.php` sink lines 95-95 (SQL_INJECTION): The sink mysqli_query($con,$q) is present and $_POST["searchitem"] is checked, but the definition and provenance of $q is not shown in the supplied source. Cannot confirm whether untrusted input reaches the query or whether any validation/prepared statement is used without assuming external code, so taint flow is indeterminate.
- `./nexus_audit_target/www/secondorder_changepass.php` sink lines 82-96 (SQL_INJECTION): The UPDATE query is built by string concatenation with $_SESSION["username"] in the WHERE clause and no prepared statement/escaping is shown. Whether $_SESSION["username"] is trustworthy and untainted cannot be proven from this snippet alone, so session trust/provenance must be assumed. Without that assumption the sink cannot be classified as definitely exploitable or definitely mitigated.
- `./nexus_audit_target/www/secondorder_home.php` sink lines 31-31 (SQL_INJECTION): The sink uses direct concatenation of $_SESSION["id"] into SQL with no prepared statement, casting, or escaping shown. Exploitability requires proof that $_SESSION["id"] is tainted by untrusted user input. The snippet does not show provenance or validation of the session value, so session trust and data origin must be assumed. Per triage rules, this requires an assumption about external validation/session trust, so state is UNKNOWN.

## Deferred: Multi-Site Taint (auto-heal not attempted)

- `./nexus_audit_target/www/blindsqli.php` sink lines 32-48 (SQL_INJECTION): query variable `$q` built via concatenation at lines 30-30; auto-heal deferred, requires manual multi-site fix
- `./nexus_audit_target/www/login1.php` sink lines 78-120 (SQL_INJECTION): query variable `$q` built via concatenation at lines 67-67; auto-heal deferred, requires manual multi-site fix
- `./nexus_audit_target/www/login2.php` sink lines 80-116 (SQL_INJECTION): query variable `$q` built via concatenation at lines 68-68; auto-heal deferred, requires manual multi-site fix
- `./nexus_audit_target/www/os_sqli.php` sink lines 33-51 (SQL_INJECTION): query variable `$q` built via concatenation at lines 31-31; auto-heal deferred, requires manual multi-site fix
- `./nexus_audit_target/www/register.php` sink lines 111-122 (SQL_INJECTION): query variable `$q` built via concatenation at lines 98-98; auto-heal deferred, requires manual multi-site fix
- `./nexus_audit_target/www/secondorder_home.php` sink lines 124-127 (SQL_INJECTION): query variable `$q` built via concatenation at lines 122-122; auto-heal deferred, requires manual multi-site fix
- `./nexus_audit_target/www/secondorder_register.php` sink lines 112-123 (SQL_INJECTION): query variable `$q` built via concatenation at lines 101-101; auto-heal deferred, requires manual multi-site fix

## Remediation Integrity

**FAIL** — One or more actionable findings were not verified fixed.

