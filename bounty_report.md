# 🛡️ Nexus Singularity Bug Bounty Report

## 🚨 Vulnerability: SQL_INJECTION
- **Severity:** High
- **File:** `./nexus_audit_target/www/secondorder_home.php` (Lines 104-127)

### Description
User-supplied input from $_REQUEST ('uid', 'name', 'descr') is concatenated directly into a SQL INSERT statement without any sanitization or use of prepared statements, allowing for SQL injection.

### Vulnerable Code
```php
		}

		?>
          
		<br />
		
      <div class="row marketing">
        <div class="col-lg-6">

<?php
//echo md5("pa55w0rd");
if (!empty($_REQUEST['uid'])) {
session_start();
$username = $_REQUEST['uid'];
$pass = $_REQUEST['password'];
$fname = $_REQUEST['name'];
$descr = $_REQUEST['descr'];

	$q = "INSERT INTO users (username, password, fname, description) values ('".$username."','".md5($pass)."','".$fname."','".$descr."')" ;
	//echo $q;
	if (!mysqli_query($con,$q))
	{
		die('Error: ' . mysqli_error($con));
	}
```

### Recommended Patch (Auto-Healed)
```php
	$stmt = mysqli_prepare($con, "INSERT INTO users (username, password, fname, description) VALUES (?, ?, ?, ?)");
	$hashed_pass = md5($pass);
	mysqli_stmt_bind_param($stmt, "ssss", $username, $hashed_pass, $fname, $descr);
	if (!mysqli_stmt_execute($stmt))
	{
		die('Error: ' . mysqli_error($con));
	}
	mysqli_stmt_close($stmt);
```

---

## 🚨 Vulnerability: SQL_INJECTION
- **Severity:** High
- **File:** `./nexus_audit_target/www/secondorder_register.php` (Lines 92-115)

### Description
The application attempts to prevent SQL injection by replacing single quotes with double single quotes (`str_replace("'","''", ...)`). However, in MySQL, this is insufficient because it does not escape backslashes. An attacker can provide a backslash (`\`) in the `uid` field, which escapes the closing single quote of the username value. This shifts the SQL context, causing the subsequent `password` (md5 hashed) and `name` fields to be interpreted as part of the SQL command rather than literal values, allowing for arbitrary SQL injection via the `name` or `descr` parameters.

### Vulnerable Code
```php
//echo md5("pa55w0rd");
if (!empty($_REQUEST['uid'])) {
session_start();
$username = str_replace("'","''",$_REQUEST['uid']);
$pass = $_REQUEST['password'];
$fname = str_replace("'","''",$_REQUEST['name']);
$descr = str_replace("'","''",$_REQUEST['descr']);
$id =  rand(20,100);

$q = "INSERT INTO users (id, username, password, fname, description) values (".$id.",'".$username."','".md5($pass)."','".$fname."','".$descr."')" ;
	
if (isset($_GET['debug']))
{
	if ($_GET['debug']=="true")
{
	$msg="<div style=\"border:1px solid #4CAF50; padding: 10px\">".$q."</div><br />";
	echo $msg;
	}
}

	if (!mysqli_query($con,$q))
	{
		echo('Error: ' . mysqli_error($con));
	}else{
```

### Recommended Patch (auto-heal attempted, REJECTED: SEARCH block mismatch, reverted)
```php
[secure replacement code]
```

---

## 🚨 Vulnerability: SQL_INJECTION
- **Severity:** High
- **File:** `./nexus_audit_target/www/register.php` (Lines 91-114)

### Description
Untrusted user input from $_REQUEST ('uid', 'name', 'descr') is concatenated directly into the SQL query string without any sanitization or the use of prepared statements, allowing for a classic SQL injection attack.

### Vulnerable Code
```php
//echo md5("pa55w0rd");
if (!empty($_REQUEST['uid'])) {
$username = $_REQUEST['uid'];
$pass = $_REQUEST['password'];
$fname = $_REQUEST['name'];
$descr = $_REQUEST['descr'];

	$q = "INSERT INTO users (username, password, fname, description) values ('".$username."','".md5($pass)."','".$fname."','".$descr."')" ;


if (isset($_GET['debug']))
{
	if ($_GET['debug']=="true")
{
	$msg="<div style=\"border:1px solid #4CAF50; padding: 10px\">".$q."</div><br />";
	echo $msg;
	}
}


	if (!mysqli_query($con,$q))
	{
		echo ('Error: ' . mysqli_error($con));
	}else{
```

### Recommended Patch (Auto-Healed)
```php
	$stmt = mysqli_prepare($con, "INSERT INTO users (username, password, fname, description) VALUES (?, ?, ?, ?)");
	$hashed_pass = md5($pass);
	mysqli_stmt_bind_param($stmt, "ssss", $username, $hashed_pass, $fname, $descr);

	if (!mysqli_stmt_execute($stmt))
	{
		echo ('Error: ' . mysqli_stmt_error($stmt));
	}else{
```

---

## 🚨 Vulnerability: SQL_INJECTION
- **Severity:** High
- **File:** `./nexus_audit_target/www/login2.php` (Lines 65-88)

### Description
Untrusted user input from $_REQUEST['uid'] is concatenated directly into a SQL query string without any sanitization or parameterization, allowing an attacker to manipulate the query logic (e.g., authentication bypass).

### Vulnerable Code
```php
$username = ($_REQUEST['uid']);
$pass = $_REQUEST['password'];

$q = "SELECT * FROM users where (username='".$username."') AND (password = '".md5($pass)."')" ;

if (isset($_GET['debug']))
{
	if ($_GET['debug']=="true")
{
	$msg="<div style=\"border:1px solid #4CAF50; padding: 10px\">".$q."</div><br />";
	echo $msg;
	}
}


		if (!mysqli_query($con,$q))
	{
		echo 'Error: ' . mysqli_error($con);
	}else{
	
	$result = mysqli_query($con,$q);

	if (mysqli_warning_count($con)) { 
   $e = mysqli_get_warnings($con); 
```

### Recommended Patch (Auto-Healed)
```php
$q = "SELECT * FROM users WHERE username = ? AND password = ?";
$stmt = mysqli_prepare($con, $q);
mysqli_stmt_bind_param($stmt, "ss", $username, md5($pass));

if (isset($_GET['debug']) && $_GET['debug'] == "true") {
    echo "<div style=\"border:1px solid #4CAF50; padding: 10px\">Query: $q</div><br />";
}

if (!mysqli_stmt_execute($stmt)) {
    echo 'Error: ' . mysqli_stmt_error($stmt);
} else {
    $result = mysqli_stmt_get_result($stmt);
```

---

## 🚨 Vulnerability: SQL_INJECTION
- **Severity:** High
- **File:** `./nexus_audit_target/www/login1.php` (Lines 63-86)

### Description
The variable 'username', derived directly from the untrusted '$_REQUEST['uid']' input, is concatenated into the SQL query string without any sanitization or use of prepared statements. This allows an attacker to perform a SQL Injection attack by providing a crafted input to bypass authentication or extract sensitive data from the database.

### Vulnerable Code
```php
if (!empty($_REQUEST['uid'])) {
$username = ($_REQUEST['uid']);
$pass = $_REQUEST['password'];

$q = "SELECT * FROM users where username='".$username."' AND password = '".md5($pass)."'" ;

if (isset($_GET['debug']))
{
	if ($_GET['debug']=="true")
{
	$msg="<div style=\"border:1px solid #4CAF50; padding: 10px\">".$q."</div><br />";
	echo $msg;
	}
}

		if (!mysqli_query($con,$q))
	{
		echo 'Error: ' . mysqli_error($con);
	}else{
	
	$result = mysqli_query($con,$q);

	// if (!$result) {
 //    		printf("%s\n", mysqli_error($con));
```

### Recommended Patch (auto-heal attempted, REJECTED: SEARCH block mismatch, reverted)
```php
        ...
        
```

---

## 🚨 Vulnerability: SQL_INJECTION
- **Severity:** High
- **File:** `./nexus_audit_target/www/login2.php` (Lines 60-83)

### Description
The variable 'uid' from $_REQUEST is directly concatenated into the SQL query string without any sanitization or use of prepared statements, allowing an attacker to manipulate the query logic via SQL Injection.

### Vulnerable Code
```php

<?php
//echo md5("pa55w0rd");

if (!empty($_REQUEST['uid'])) {
$username = ($_REQUEST['uid']);
$pass = $_REQUEST['password'];

$q = "SELECT * FROM users where (username='".$username."') AND (password = '".md5($pass)."')" ;

if (isset($_GET['debug']))
{
	if ($_GET['debug']=="true")
{
	$msg="<div style=\"border:1px solid #4CAF50; padding: 10px\">".$q."</div><br />";
	echo $msg;
	}
}


		if (!mysqli_query($con,$q))
	{
		echo 'Error: ' . mysqli_error($con);
	}else{
```

### Recommended Patch (auto-heal skipped: site already resolved by an earlier patch in this run)

---

## 🚨 Vulnerability: SQL_INJECTION
- **Severity:** High
- **File:** `./nexus_audit_target/www/login1.php` (Lines 58-81)

### Description
The application directly concatenates user-supplied input from '$_REQUEST['uid']' into a SQL query string without any sanitization or use of prepared statements, leading to a classic SQL injection vulnerability.

### Vulnerable Code
```php
        <div class="col-lg-6">

<?php
//echo md5("pa55w0rd");

if (!empty($_REQUEST['uid'])) {
$username = ($_REQUEST['uid']);
$pass = $_REQUEST['password'];

$q = "SELECT * FROM users where username='".$username."' AND password = '".md5($pass)."'" ;

if (isset($_GET['debug']))
{
	if ($_GET['debug']=="true")
{
	$msg="<div style=\"border:1px solid #4CAF50; padding: 10px\">".$q."</div><br />";
	echo $msg;
	}
}

		if (!mysqli_query($con,$q))
	{
		echo 'Error: ' . mysqli_error($con);
	}else{
```

### Recommended Patch (auto-heal attempted, REJECTED: Syntax check failed, reverted)
```php
$username = $_REQUEST['uid'];
$pass = md5($_REQUEST['password']);

$stmt = mysqli_prepare($con, "SELECT * FROM users WHERE username = ? AND password = ?");
mysqli_stmt_bind_param($stmt, "ss", $username, $pass);

if (isset($_GET['debug']) && $_GET['debug'] == "true") {
	echo "<div style=\"border:1px solid #4CAF50; padding: 10px\">Executing prepared statement for user: " . htmlspecialchars($username) . "</div><br />";
}

if (!mysqli_stmt_execute($stmt)) {
```

---

## 🚨 Vulnerability: SQL_INJECTION
- **Severity:** High
- **File:** `./nexus_audit_target/www/blindsqli.php` (Lines 16-39)

### Description
The application takes untrusted user input from $_GET['user'] and concatenates it directly into a SQL query string without any sanitization or use of prepared statements, allowing an attacker to manipulate the query structure.

### Vulnerable Code
```php
	</head>

  <body>
  <div class="container-narrow">
		
		<div class="jumbotron">
			<p class="lead" style="color:white">
				Blind SQL Injection (via content response and time delays)</a>
				</p>
		</div>

		<?php
if (isset($_GET["user"])){
		$user = $_GET["user"];
		$q = "Select * from users where username = '".$user."'";

		if (!mysqli_query($con,$q))
	{
		//die('Error: ' . mysqli_error($con));
	}else{
		$result = mysqli_query($con,$q);
		

		$row = mysqli_fetch_array($result);
```

### Recommended Patch (auto-heal attempted, REJECTED: Syntax check failed, reverted)
```php
if (isset($_GET["user"])){
		$user = $_GET["user"];
		$stmt = mysqli_prepare($con, "SELECT * FROM users WHERE username = ?");
		mysqli_stmt_bind_param($stmt, "s", $user);
		mysqli_stmt_execute($stmt);
		$result = mysqli_stmt_get_result($stmt);
```

---

## 🚨 Vulnerability: SQL_INJECTION
- **Severity:** High
- **File:** `./nexus_audit_target/www/os_sqli.php` (Lines 13-36)

### Description
Untrusted user input from $_GET['user'] is directly concatenated into a SQL query string without any sanitization or parameterization, and then executed using mysqli_multi_query, which allows for the execution of multiple SQL statements.

### Vulnerable Code
```php
<html lang="en"><head><meta http-equiv="Content-Type" content="text/html; charset=UTF-8">
    <title>OS interaction using SQL Injection - SQL Injection Training App</title>

    <link href="./css/htmlstyles.css" rel="stylesheet">
	</head>

  <body>
  <div class="container-narrow">
		
		<div class="jumbotron">
			<p class="lead" style="color:white">
				OS interaction using SQL Injection - Let's drop a shell!</a>
				</p>
		</div>
		
<?php
if (isset($_GET["user"])){
		$user = $_GET["user"];
		$q = "Select * from users where username = '".$user."'";

		if (mysqli_multi_query($con,$q))
		{
		  do
			{
```

### Recommended Patch (auto-heal attempted, REJECTED: SEARCH block mismatch, reverted)
```php
[secure replacement code]
```

---

## 🚨 Vulnerability: SQL_INJECTION
- **Severity:** High
- **File:** `./nexus_audit_target/www/blindsqli.php` (Lines 12-35)

### Description
The application takes user input directly from the 'user' GET parameter and concatenates it into a SQL query string without any sanitization or parameterization. This allows an attacker to manipulate the SQL query by injecting malicious SQL commands.

### Vulnerable Code
```php
<html lang="en"><head><meta http-equiv="Content-Type" content="text/html; charset=UTF-8">
    <title>Blind SQL Injection - SQL Injection Training App</title>

    <link href="./css/htmlstyles.css" rel="stylesheet">
	</head>

  <body>
  <div class="container-narrow">
		
		<div class="jumbotron">
			<p class="lead" style="color:white">
				Blind SQL Injection (via content response and time delays)</a>
				</p>
		</div>

		<?php
if (isset($_GET["user"])){
		$user = $_GET["user"];
		$q = "Select * from users where username = '".$user."'";

		if (!mysqli_query($con,$q))
	{
		//die('Error: ' . mysqli_error($con));
	}else{
```

### Recommended Patch (auto-heal attempted, REJECTED: SEARCH block mismatch, reverted)
```php
`, `
```

---


# Final Audit Summary

- **Actionable findings:** 10
- **Verified fixed:** 3
- **Remediation failed:** 6
- **Explicitly safe:** 1
- **Unknown:** 5

## Remediation Integrity

                             **FAIL** — One or more actionable findings were not VERIFIED_FIXED.

