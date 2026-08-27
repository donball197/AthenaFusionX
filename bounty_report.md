# 🛡️ Nexus Singularity Bug Bounty Report

## 🚨 Vulnerability: SQL_INJECTION
- **Severity:** High
- **File:** `./nexus_audit_target/www/blindsqli.php` (Sink lines 36-36)

### Description
The code constructs a SQL query by directly concatenating the user-supplied 'username' parameter into the WHERE clause. This lack of parameterization or escaping allows an attacker to inject arbitrary SQL code, potentially bypassing authentication or extracting sensitive data from the database.

### Vulnerable Code
```php
		$result = mysqli_query($con,$q);
```

### Recommended Patch (Auto-Healed, Verified)
```php
$stmt = $con->prepare("SELECT * FROM users WHERE username = ?");
$stmt->bind_param("s", $user);
if (!$stmt->execute()) {
    //die('Error: ' . mysqli_error($con));
} else {
    $result = $stmt->get_result();
```

---

## 🚨 Vulnerability: SQL_INJECTION
- **Severity:** High
- **File:** `./nexus_audit_target/www/blindsqli.php` (Sink lines 32-32)

### Description
The code constructs a SQL query by directly concatenating the user-supplied 'username' parameter into the WHERE clause. This lack of parameterization or escaping allows an attacker to inject arbitrary SQL code, potentially leading to data leakage or manipulation.

### Vulnerable Code
```php
		if (!mysqli_query($con,$q))
```

### Patch (skipped: site already resolved by an earlier patch in this run)

---

## 🚨 Vulnerability: SQL_INJECTION
- **Severity:** High
- **File:** `./nexus_audit_target/www/login1.php` (Sink lines 83-83)

### Description
The code constructs a SQL query by directly concatenating user-supplied values (username and password) into the string. This lack of parameterization or escaping allows for SQL injection, where an attacker could manipulate the query to bypass authentication or extract data.

### Vulnerable Code
```php
	$result = mysqli_query($con,$q);
```

### Recommended Patch (REJECTED: tree-sitter syntax gate failed - never written)
```php
$stmt = $con->prepare("SELECT * FROM users WHERE username = ? AND password = ?");
$stmt->bind_param("ss", $username, md5($pass));
$stmt->execute();

if (isset($_GET['debug'])) {
    if ($_GET['debug'] == "true") {
        $msg = "<div style=\"border:1px solid #4CAF50; padding: 10px\">" . $stmt->sql . "</div><br />";
        echo $msg;
    }
}

if (!$stmt) {
    echo 'Error: ' . mysqli_error($con);
} else {
    $result = $stmt->get_result();
}
```
```

---

## 🚨 Vulnerability: SQL_INJECTION
- **Severity:** High
- **File:** `./nexus_audit_target/www/login1.php` (Sink lines 78-78)

### Description
The code constructs a SQL query by directly concatenating user-supplied values (username and password) into the WHERE clause. This lack of parameterization or escaping makes the system highly vulnerable to SQL injection attacks, where an attacker could manipulate the query to bypass authentication or extract sensitive data.

### Vulnerable Code
```php
		if (!mysqli_query($con,$q))
```

### Recommended Patch (REJECTED: tree-sitter syntax gate failed - never written)
```php
$stmt = $con->prepare("SELECT * FROM users WHERE username = ? AND password = ?");
$stmt->bind_param("ss", $username, md5($pass));
$stmt->execute();
$result = $stmt->get_result();

if (isset($_GET['debug']) && $_GET['debug'] == "true") {
    $msg = "<div style=\"border:1px solid #4CAF50; padding: 10px\">" . $stmt->error . "</div><br />";
    echo $msg;
}

if ($stmt->error) {
    echo 'Error: ' . $stmt->error;
} else {
    if (mysqli_warning_count($con)) {
        $e = mysqli_get_warnings($con);
        if ($e) {
            do {
                echo "Warning: $e->errno: $e->message\n";
            } while ($e->next());
        }
    }

    echo "<br /><br />";
    $row = $result->fetch_array();

    if ($row) {
        $_SESSION["username"] = $row[1];
        $_SESSION["name"] = $row[3];

        if ($_SESSION['next'] == "searchproducts.php") {
            header('Location: searchproducts.php');
        } elseif ($_SESSION['next'] == "blindsqli.php") {
            header('Location: blindsqli.php?user=' . $_SESSION["username"]);
        } elseif ($_SESSION['next'] == "os_sqli.php") {
            header('Location: os_sqli.php?user=' . $_SESSION["username"]);
        }
    } else {
        echo "<font style=\"color:#FF0000\">Invalid password!</font>";
    }
}
```
```

---

## 🚨 Vulnerability: SQL_INJECTION
- **Severity:** High
- **File:** `./nexus_audit_target/www/login2.php` (Sink lines 85-85)

### Description
The code constructs a SQL query by directly concatenating user-supplied values (username and password) into the string, without using prepared statements or proper sanitization. This makes the application vulnerable to SQL injection attacks, where an attacker could manipulate the query to bypass authentication or extract sensitive data.

### Vulnerable Code
```php
	$result = mysqli_query($con,$q);
```

### Recommended Patch (REJECTED: tree-sitter syntax gate failed - never written)
```php
$stmt = $con->prepare("SELECT * FROM users WHERE username = ? AND password = ?");
$stmt->bind_param("ss", $username, md5($pass));
$stmt->execute();
$result = $stmt->get_result();
```
```

---

## 🚨 Vulnerability: SQL_INJECTION
- **Severity:** High
- **File:** `./nexus_audit_target/www/login2.php` (Sink lines 80-80)

### Description
The code constructs a SQL query by concatenating user-supplied input (username and password) directly into the query string. This lack of parameterization or escaping allows an attacker to inject arbitrary SQL commands, potentially leading to data leakage, modification, or denial of service.

### Vulnerable Code
```php
		if (!mysqli_query($con,$q))
```

### Recommended Patch (REJECTED: tree-sitter syntax gate failed - never written)
```php
$stmt = $con->prepare("SELECT * FROM users WHERE username = ? AND password = ?");
$stmt->bind_param("ss", $username, md5($pass));
$stmt->execute();
$result = $stmt->get_result();

if (mysqli_warning_count($con)) {
   $e = mysqli_get_warnings($con);
   if ($e) {
       do {
           echo "Warning: $e->errno: $e->message\n";
       } while ($e->next());
   }
}

echo "<br /><br />";
$row = mysqli_fetch_array($result);

if ($row) {
   $_SESSION["username"] = $row[1];
   $_SESSION["name"] = $row[3];
   if ($_SESSION['next'] == "searchproducts.php") {
       header('Location: searchproducts.php');
   } elseif ($_SESSION['next'] == "blindsqli.php") {
       header('Location: blindsqli.php?user=' . $_SESSION["username"]);
   } elseif ($_SESSION['next'] == "os_sqli.php") {
       header('Location: os_sqli.php?user=' . $_SESSION["username"]);
   }
} else {
   echo "<font style=\"color:#FF0000\">Invalid password!</font>";
}
```
```

---

## 🚨 Vulnerability: SQL_INJECTION
- **Severity:** High
- **File:** `./nexus_audit_target/www/os_sqli.php` (Sink lines 33-33)

### Description
The code constructs a SQL query by directly concatenating the user-supplied 'user' parameter into the WHERE clause, creating a classic SQL injection vulnerability. An attacker can manipulate the 'user' parameter to execute arbitrary SQL commands, potentially gaining unauthorized access to the database or the underlying operating system.

### Vulnerable Code
```php
		if (mysqli_multi_query($con,$q))
```

### Recommended Patch (Auto-Healed, Verified)
```php
$stmt = $con->prepare("SELECT * FROM users WHERE username = ?");
$stmt->bind_param("s", $user);
$stmt->execute();
$result = $stmt->get_result();
while ($row = $result->fetch_assoc()) {
    $username = $row['username'];
    $name = $row['name'];
    $descr = $row['description'];
}
$stmt->close();
```

---

## 🚨 Vulnerability: SQL_INJECTION
- **Severity:** High
- **File:** `./nexus_audit_target/www/register.php` (Sink lines 111-111)

### Description
The code constructs a SQL query by directly concatenating user-supplied input (from $_REQUEST) into the string, without any sanitization or parameterization. This makes the application highly vulnerable to SQL injection attacks, where an attacker could manipulate the query to execute arbitrary SQL commands.

### Vulnerable Code
```php
	if (!mysqli_query($con,$q))
```

### Recommended Patch (Auto-Healed, Verified)
```php
$stmt = $con->prepare("INSERT INTO users (username, password, fname, description) VALUES (?, ?, ?, ?)");
	$stmt->bind_param("ssss", $username, md5($pass), $fname, $descr);

	if (isset($_GET['debug'])) {
		if ($_GET['debug'] == "true") {
			$msg = "<div style=\"border:1px solid #4CAF50; padding: 10px\">" . $q . "</div><br />";
			echo $msg;
		}
	}

	if (!$stmt->execute()) {
		echo ('Error: ' . $stmt->error);
	} else {
		$_SESSION["username"] = $username;
		$_SESSION["fname"] = $fname;

		ob_clean();
		header('Location:searchproducts.php');
	}
```

---

## 🚨 Vulnerability: SQL_INJECTION
- **Severity:** High
- **File:** `./nexus_audit_target/www/secondorder_changepass.php` (Sink lines 87-87)

### Description
The code constructs a SQL update query by concatenating user-supplied values (specifically the new password) directly into the string. This lack of parameterization or escaping makes the system vulnerable to SQL injection attacks, where an attacker could manipulate the query to change passwords for other users or execute arbitrary database commands.

### Vulnerable Code
```php
	$result = mysqli_query($con,$q);
```

### Recommended Patch (Auto-Healed, Verified)
```php
$stmt = $con->prepare("UPDATE users SET password = ? WHERE username = ?");
$stmt->bind_param("ss", md5($npass1), $_SESSION["username"]);
if (!$stmt->execute()) {
    echo('Error: ' . $stmt->error);
} else {
    $result = $stmt->get_result();
```

---

## 🚨 Vulnerability: SQL_INJECTION
- **Severity:** High
- **File:** `./nexus_audit_target/www/secondorder_home.php` (Sink lines 124-124)

### Description
The code constructs an SQL query by concatenating user-supplied values (username, password, fname, descr) directly into the string. This lack of parameterization or escaping makes the system highly vulnerable to SQL injection attacks, where malicious users can manipulate the query to extract data or alter the database.

### Vulnerable Code
```php
	if (!mysqli_query($con,$q))
```

### Recommended Patch (Auto-Healed, Verified)
```php
$stmt = $con->prepare("INSERT INTO users (username, password, fname, description) VALUES (?, ?, ?, ?)");
	$stmt->bind_param("ssss", $username, md5($pass), $fname, $descr);
	if (!$stmt->execute()) {
		die('Error: ' . $stmt->error);
	}
```

---

## 🚨 Vulnerability: SQL_INJECTION
- **Severity:** High
- **File:** `./nexus_audit_target/www/secondorder_register.php` (Sink lines 112-112)

### Description
The code constructs a SQL query by concatenating user-supplied data (username, password, fname, descr) directly into the string, without using prepared statements or proper parameterization. This makes the application vulnerable to SQL injection attacks, where an attacker could manipulate the query to execute arbitrary SQL commands or extract sensitive data.

### Vulnerable Code
```php
	if (!mysqli_query($con,$q))
```

### Recommended Patch (REJECTED: tree-sitter syntax gate failed - never written)
```php
$stmt = $con->prepare("INSERT INTO users (id, username, password, fname, description) VALUES (?, ?, ?, ?, ?)");
$stmt->bind_param("issss", $id, $username, md5($pass), $fname, $descr);

if (isset($_GET['debug']) && $_GET['debug'] == "true") {
    $msg = "<div style=\"border:1px solid #4CAF50; padding: 10px\">" . $stmt->query . "</div><br />";
    echo $msg;
}

if (!$stmt->execute()) {
    echo 'Error: ' . $stmt->error;
} else {
    $_SESSION["id"] = $id;
    $_SESSION["fname"] = $fname;
    ob_clean();
    header('Location:secondorder_home.php');
}
```
```

---


# Final Audit Summary

- **Actionable findings:** 11
- **Verified fixed:** 5
- **Remediation failed:** 5
- **Already resolved (earlier patch in this run):** 1
- **Explicitly safe:** 1
- **Unknown (needs manual review):** 3
- **Deferred (multi-site taint, not patched):** 0
- **Triage errors (API):** 0

## Needs Manual Review

- `./nexus_audit_target/www/searchproducts.php` sink lines 95-95 (SQL_INJECTION): The code snippet shows a SQL query being executed with user-provided data (from a POST request), but the actual query string ($q) is not displayed. Without seeing the specific SQL statement and how the user input is incorporated (e.g., via concatenation or prepared statements), it is impossible to definitively determine if the query is vulnerable to SQL injection or if it is properly mitigated. Therefore, the state is classified as UNKNOWN.
- `./nexus_audit_target/www/secondorder_changepass.php` sink lines 107-107 (SQL_INJECTION): The code snippet shows a SQL query being constructed with a session variable ($_SESSION['username']) embedded directly into the string. While the session variable is trusted, the lack of explicit sanitization or prepared statements means the vulnerability status cannot be definitively determined without knowing the context of how the session was established and the data integrity of the username field.
- `./nexus_audit_target/www/secondorder_home.php` sink lines 31-31 (SQL_INJECTION): The code uses a session variable (`$_SESSION['id']`) as the source for the SQL query, which is not directly controlled by the user in this context. However, without knowing the provenance of this session data (e.g., how it was set and whether it was validated), it is impossible to definitively determine if the sink is safe or exploitable. The lack of explicit data validation or prepared statements suggests potential risk, but the indirect nature of the input makes a conclusive classification as 'SAFE' or 'EXPLOITABLE' unreliable without further context.

## Remediation Integrity

**FAIL** — One or more actionable findings were not verified fixed.

